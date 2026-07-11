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

        KScreenGapGuard.Restore(_factory.QueryMonitors());

        if (!UseKScreen)
            return Notify(Run("xrandr", string.Join(' ', moved.Select(l =>
                $"--output {l.Source.InterfacePath} --pos {RoundI(l.Position.X)}x{RoundI(l.Position.Y)}"))));

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

        if (!RunKScreen(string.Join(' ', moved.Select(PositionArg))))
            return false;

        // Trust but verify: re-query and re-assert once if the compositor moved
        // anything away from the requested spot while settling the config. A 1px
        // drift here silently becomes an overlapping/gapped edge — the exact
        // topology corruption the mouse engine cannot paper over.
        var expected = moved.ToDictionary(
            l => l.Source.InterfacePath,
            l => (X: RoundI(l.Position.X), Y: RoundI(l.Position.Y)));
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

        return Notify(true);
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
