#nullable enable
using System;
using System.Diagnostics;
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
    public void PrepareForEngine()
    {
        if (KScreenGapGuard.Apply(_factory.QueryMonitors()))
            _factory.NotifyDisplayChanged();
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
