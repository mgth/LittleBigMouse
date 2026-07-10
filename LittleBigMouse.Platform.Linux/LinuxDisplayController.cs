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
    /// One kscreen-doctor (or xrandr) invocation for the whole batch. Engine gaps are
    /// closed first — Restore deletes the journal, so the old positions can never be
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

        return Notify(UseKScreen
            ? RunKScreen(string.Join(' ', moved.SelectMany(KScreenArgs)))
            : Run("xrandr", string.Join(' ', moved.Select(l =>
                $"--output {l.Source.InterfacePath} --pos {(int)l.Position.X}x{(int)l.Position.Y}"))));
    }

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

    static IEnumerable<string> KScreenArgs((DisplaySource Source, Point Position, double? Scale) l)
    {
        yield return $"output.{l.Source.InterfacePath}.position.{(int)l.Position.X},{(int)l.Position.Y}";
        // xrandr has no fractional per-output scale: the option only exists on kscreen.
        if (l.Scale is { } scale)
            yield return $"output.{l.Source.InterfacePath}.scale.{scale.ToString(CultureInfo.InvariantCulture)}";
    }

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
