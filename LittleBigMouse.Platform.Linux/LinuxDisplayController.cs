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
    static bool UseKScreen
        => Environment.GetEnvironmentVariable("XDG_CURRENT_DESKTOP")?.Contains("KDE", StringComparison.OrdinalIgnoreCase) == true;

    public bool SetPrimary(DisplaySource source)
        => UseKScreen
            // Plasma 6 replaced the primary flag by a priority order: 1 is the primary.
            ? Run("kscreen-doctor", $"output.{source.InterfacePath}.priority.1")
            : Run("xrandr", $"--output {source.InterfacePath} --primary");

    public bool AttachToDesktop(DisplaySource source)
        => UseKScreen
            ? Run("kscreen-doctor", $"output.{source.InterfacePath}.enable")
            : Run("xrandr", $"--output {source.InterfacePath} --auto");

    public bool DetachFromDesktop(DisplaySource source)
        => UseKScreen
            ? Run("kscreen-doctor", $"output.{source.InterfacePath}.disable")
            : Run("xrandr", $"--output {source.InterfacePath} --off");

    static bool Run(string command, string arguments)
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
