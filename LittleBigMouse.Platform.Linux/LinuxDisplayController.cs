#nullable enable
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Linq;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Linux implementation of <see cref="IDisplayController"/> through kscreen-doctor
/// (Plasma). Best-effort by design: failures are logged, never thrown — the layout
/// builder's core loop only needs discovery, topology mutation is a convenience.
/// <c>DisplaySource.InterfacePath</c> holds the connector name (e.g. "DP-2"), which is
/// exactly the kscreen-doctor output identifier. Non-KDE sessions fall back to xrandr.
/// </summary>
public class LinuxDisplayController : IDisplayController
{
    readonly LinuxLayoutFactory _factory;

    public LinuxDisplayController(LinuxLayoutFactory factory)
    {
        _factory = factory;

        // A previous run may have died while the topology was gapped for the engine
        // (kill -9, power loss): put the outputs back before anything is built on top.
        try
        {
            if (KScreenGapGuard.RecoverStale(factory.QueryMonitors()))
                factory.NotifyDisplayChanged();
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"Gap recovery failed: {ex.Message}");
        }
    }

    /// <summary>
    /// Engine prologue/epilogue (see <see cref="KScreenGapGuard"/>): open 1px gaps at
    /// shared edges so the daemon's barriers pass the compositor validator, and close
    /// them once the engine stops. Any actual change goes through NotifyDisplayChanged
    /// so MainService rebuilds and re-sends zones computed in the new coordinate space.
    /// </summary>
    public bool PrepareForEngine()
    {
        if (!KScreenGapGuard.Apply(_factory.QueryMonitors())) return false;
        _factory.NotifyDisplayChanged();
        return true;
    }

    public void RestoreAfterEngine()
    {
        if (KScreenGapGuard.Restore(_factory.QueryMonitors()))
            _factory.NotifyDisplayChanged();
    }

    static bool UseKScreen
        => Environment.GetEnvironmentVariable("XDG_CURRENT_DESKTOP")?.Contains("KDE", StringComparison.OrdinalIgnoreCase) == true;

    public bool SetPrimary(DisplaySource source)
        => Notify(UseKScreen
            // Plasma 6 replaced the primary flag by a priority order: 1 is the primary.
            ? Run("kscreen-doctor", $"output.{source.InterfacePath}.priority.1")
            : Run("xrandr", $"--output {source.InterfacePath} --primary"));

    public bool AttachToDesktop(DisplaySource source)
        => Notify(UseKScreen
            ? Run("kscreen-doctor", $"output.{source.InterfacePath}.enable")
            : Run("xrandr", $"--output {source.InterfacePath} --auto"));

    public bool DetachFromDesktop(DisplaySource source)
        => Notify(UseKScreen
            ? Run("kscreen-doctor", $"output.{source.InterfacePath}.disable")
            : Run("xrandr", $"--output {source.InterfacePath} --off"));

    /// <summary>
    /// Apply the batch of positions (and optional scales). Engine gaps are closed
    /// first — Restore deletes the journal, so the old positions can never be
    /// "restored" over the ones applied here; if the engine is running, the rebuild
    /// triggered by Notify re-runs PrepareForEngine which re-journals and re-gaps the
    /// new topology.
    /// </summary>
    public bool SetLocations(IReadOnlyList<(DisplaySource Source, Point Position, double? Scale)> locations)
    {
        if (locations.Count == 0) return true;

        // kscreen refuses negative positions for enabled outputs, so the Windows-style
        // primary-at-(0,0) anchor cannot be applied verbatim: translate the whole
        // topology so its top-left corner is (0,0), wherever that puts the primary.
        var dx = locations.Min(l => l.Position.X);
        var dy = locations.Min(l => l.Position.Y);
        var moved = locations
            .Select(l => (l.Source, Position: new Point(l.Position.X - dx, l.Position.Y - dy), l.Scale))
            .Where(l => l.Scale.HasValue || l.Position != l.Source.InPixel.Bounds.Location)
            .ToList();

        // Nothing to change: skip Restore too, an engine running with its gaps keeps them.
        if (moved.Count == 0) return true;

        var monitorsBefore = _factory.QueryMonitors();
        KScreenGapGuard.Restore(monitorsBefore);

        if (!UseKScreen)
            return Notify(Run("xrandr", string.Join(' ', moved.Select(l =>
                $"--output {l.Source.InterfacePath} --pos {RoundI(l.Position.X)}x{RoundI(l.Position.Y)}"))));

        // The logical sizes the solver assumed (round(native/scale), same rounding
        // as the adapter) — the positions in `moved` are only consistent with THESE.
        var before = monitorsBefore.Where(m => m.Enabled).ToDictionary(m => m.ConnectorName);
        var placed = moved.Select(l =>
        {
            before.TryGetValue(l.Source.InterfacePath, out var b);
            double predictedW = b?.LogicalWidth ?? l.Source.InPixel.Width;
            double predictedH = b?.LogicalHeight ?? l.Source.InPixel.Height;
            if (b is not null && l.Scale is { } s && Math.Abs(s - b.Scale) >= 1.0 / 240)
            {
                predictedW = Math.Round(b.PixelWidth / s);
                predictedH = Math.Round(b.PixelHeight / s);
            }
            return new PlacedOutput(l.Source.InterfacePath,
                l.Position.X, l.Position.Y, predictedW, predictedH, predictedW, predictedH);
        }).ToList();

        // Scales go in their own pass, FIRST: a scale change resizes the output's
        // logical rect and lets the compositor re-shuffle its neighbours to its own
        // taste (observed live: a 1px overlap appearing at a shared edge). Whatever
        // it decides, the positions pass below must be the last word.
        var scales = moved
            .Where(l => l.Scale.HasValue)
            .Select(l => $"output.{l.Source.InterfacePath}.scale.{l.Scale!.Value.ToString(CultureInfo.InvariantCulture)}")
            .ToList();
        if (scales.Count > 0 && !RunKScreen(string.Join(' ', scales)))
            return false;

        // The compositor's own rounding of native/scale is authoritative and can
        // differ by a pixel from the prediction — enough to turn an intended edge
        // contact into an overlap or a gap that faithful re-assertion would then
        // preserve. Re-read the actual sizes and rebuild the contact chains.
        if (scales.Count > 0)
        {
            var actual = _factory.QueryMonitors().Where(m => m.Enabled).ToDictionary(m => m.ConnectorName);
            placed = placed.Select(p => actual.TryGetValue(p.Name, out var m)
                    ? p with { ActualWidth = m.LogicalWidth, ActualHeight = m.LogicalHeight }
                    : p)
                .ToList();
        }

        var snapped = SnapToActualSizes(placed);

        if (!RunKScreen(string.Join(' ', placed.Select(p =>
                $"output.{p.Name}.position.{RoundI(snapped[p.Name].X)},{RoundI(snapped[p.Name].Y)}"))))
            return false;

        // Trust but verify: re-query and re-assert once if the compositor moved
        // anything away from the requested spot while settling the config. A 1px
        // drift here silently becomes an overlapping/gapped edge — the exact
        // topology corruption the mouse engine cannot paper over.
        var expected = placed.ToDictionary(
            p => p.Name,
            p => (X: RoundI(snapped[p.Name].X), Y: RoundI(snapped[p.Name].Y)));
        for (var attempt = 0; ; attempt++)
        {
            var live = _factory.QueryMonitors()
                .Where(m => m.Enabled)
                .ToDictionary(m => m.ConnectorName);
            var drifted = expected
                .Where(e => live.TryGetValue(e.Key, out var m)
                            && (RoundI(m.LogicalX) != e.Value.X || RoundI(m.LogicalY) != e.Value.Y))
                .ToList();
            if (drifted.Count == 0) break;
            if (attempt >= 1)
            {
                Debug.WriteLine("SetLocations: compositor kept overriding positions: "
                    + string.Join(", ", drifted.Select(d => d.Key)));
                break;
            }
            RunKScreen(string.Join(' ', drifted.Select(d => $"output.{d.Key}.position.{d.Value.X},{d.Value.Y}")));
        }

        AuditOverlaps();

        return Notify(true);
    }

    /// <summary>
    /// One output placed by the solver: intended position with the sizes the solver
    /// believed in, plus the sizes the compositor actually settled on.
    /// </summary>
    public record PlacedOutput(
        string Name,
        double X, double Y,
        double PredictedWidth, double PredictedHeight,
        double ActualWidth, double ActualHeight);

    /// <summary>
    /// Rebuild the intended edge contacts with the actual output sizes: wherever the
    /// solver meant two outputs to touch (edge-to-edge in its own predicted space),
    /// chain them flush using the sizes the compositor really applied. Outputs
    /// without a contact on an axis keep their intended coordinate.
    /// </summary>
    public static Dictionary<string, (double X, double Y)> SnapToActualSizes(IReadOnlyList<PlacedOutput> placed)
    {
        const double contactTolerance = 1.5;

        // does the pair share an edge span on the perpendicular axis, in intended space?
        static bool Overlap(double aLo, double aLength, double bLo, double bLength)
            => Math.Min(aLo + aLength, bLo + bLength) - Math.Max(aLo, bLo) > 0.5;

        var result = placed.ToDictionary(p => p.Name, p => (p.X, p.Y));

        // A found contact REPLACES the intended coordinate (that closes gaps as
        // well as overlaps); the max only arbitrates between several neighbours.
        foreach (var item in placed.OrderBy(p => p.X).ThenBy(p => p.Y))
        {
            double? chained = null;
            foreach (var prior in placed)
            {
                if (prior.Name == item.Name) continue;
                if (!Overlap(prior.Y, prior.PredictedHeight, item.Y, item.PredictedHeight)) continue;
                if (Math.Abs(prior.X + prior.PredictedWidth - item.X) > contactTolerance) continue;
                var x = result[prior.Name].X + prior.ActualWidth;
                chained = chained is null ? x : Math.Max(chained.Value, x);
            }
            result[item.Name] = (chained ?? item.X, result[item.Name].Y);
        }

        foreach (var item in placed.OrderBy(p => p.Y).ThenBy(p => p.X))
        {
            double? chained = null;
            foreach (var prior in placed)
            {
                if (prior.Name == item.Name) continue;
                if (!Overlap(prior.X, prior.PredictedWidth, item.X, item.PredictedWidth)) continue;
                if (Math.Abs(prior.Y + prior.PredictedHeight - item.Y) > contactTolerance) continue;
                var y = result[prior.Name].Y + prior.ActualHeight;
                chained = chained is null ? y : Math.Max(chained.Value, y);
            }
            result[item.Name] = (result[item.Name].X, chained ?? item.Y);
        }

        return result;
    }

    /// <summary>
    /// Post-apply sanity check over EVERY enabled output — including ones this batch
    /// never touched, which keep whatever stale position the compositor had for them
    /// (observed live: a rarely-used output overlapping a neighbour by 1280px).
    /// </summary>
    void AuditOverlaps()
    {
        try
        {
            var live = _factory.QueryMonitors().Where(m => m.Enabled).ToList();
            for (var i = 0; i < live.Count; i++)
            for (var j = i + 1; j < live.Count; j++)
            {
                var a = live[i];
                var b = live[j];
                var w = Math.Min(a.LogicalX + a.LogicalWidth, b.LogicalX + b.LogicalWidth) - Math.Max(a.LogicalX, b.LogicalX);
                var h = Math.Min(a.LogicalY + a.LogicalHeight, b.LogicalY + b.LogicalHeight) - Math.Max(a.LogicalY, b.LogicalY);
                if (w > 0.5 && h > 0.5)
                    Console.Error.WriteLine(
                        $"SetLocations: outputs overlap after apply: {a.ConnectorName} " +
                        $"({a.LogicalX},{a.LogicalY} {a.LogicalWidth}x{a.LogicalHeight}) / {b.ConnectorName} " +
                        $"({b.LogicalX},{b.LogicalY} {b.LogicalWidth}x{b.LogicalHeight})");
            }
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"Overlap audit failed: {ex.Message}");
        }
    }

    static int RoundI(double v) => (int)Math.Round(v);

    /// <summary>
    /// kscreen-doctor exits 0 even when the compositor rejects the config: the only
    /// failure signal is "applying config failed!" on its output.
    /// </summary>
    static bool RunKScreen(string arguments)
    {
        try
        {
            using var process = Process.Start(new ProcessStartInfo("kscreen-doctor", arguments)
            {
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false
            });
            if (process == null) return false;

            var output = process.StandardOutput.ReadToEnd() + process.StandardError.ReadToEnd();
            process.WaitForExit(10000);

            if (process.ExitCode == 0 && !output.Contains("failed", StringComparison.OrdinalIgnoreCase))
                return true;

            Debug.WriteLine($"kscreen-doctor {arguments} failed: {output}");
            return false;
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"kscreen-doctor {arguments} failed: {ex.Message}");
            return false;
        }
    }

    static string PositionArg((DisplaySource Source, Point Position, double? Scale) l)
        => $"output.{l.Source.InterfacePath}.position.{RoundI(l.Position.X)},{RoundI(l.Position.Y)}";

    /// <summary>
    /// A successful topology command must trigger the UI rebuild itself: primary,
    /// enable/disable and position changes are invisible to the sysfs plug poll
    /// (on Windows the equivalent WM_DISPLAYCHANGE arrives through the daemon).
    /// </summary>
    bool Notify(bool success)
    {
        if (success) _factory.NotifyDisplayChanged();
        return success;
    }

    internal static bool Run(string command, string arguments)
    {
        try
        {
            using var process = Process.Start(new ProcessStartInfo(command, arguments)
            {
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false
            });
            if (process == null) return false;
            process.WaitForExit(10000);

            if (process.ExitCode == 0) return true;

            Debug.WriteLine($"{command} {arguments} failed: {process.StandardError.ReadToEnd()}");
            return false;
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"{command} {arguments} failed: {ex.Message}");
            return false;
        }
    }
}
