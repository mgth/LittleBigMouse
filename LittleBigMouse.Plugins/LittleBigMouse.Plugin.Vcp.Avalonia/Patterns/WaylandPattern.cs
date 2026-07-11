#nullable enable
using System.Diagnostics;
using System.IO;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Patterns;

/// <summary>
/// Drives the lbm-pattern helper: a bare native-Wayland client that fullscreens
/// a PNG on a given output with a wl_shm buffer at the panel's native
/// resolution (wp_viewport maps it onto the logical size, 1:1 when they
/// match). Needed because the Avalonia windows live on XWayland, which the
/// compositor rescales on any output whose scale differs from the global
/// factor — destroying pixel-accurate patterns like the gamma checkerboard.
/// </summary>
internal static class WaylandPattern
{
    const string HelperName = "lbm-pattern";

    static string? _helper;
    static bool _searched;

    public static bool IsAvailable =>
        OperatingSystem.IsLinux()
        && !string.IsNullOrEmpty(Environment.GetEnvironmentVariable("WAYLAND_DISPLAY"))
        && FindHelper() is not null;

    /// <summary>Connected outputs by connector name, with their native pixel mode.</summary>
    public static Dictionary<string, (int Width, int Height)> ListOutputs()
    {
        var outputs = new Dictionary<string, (int, int)>();
        var helper = FindHelper();
        if (helper is null) return outputs;

        try
        {
            using var process = Process.Start(new ProcessStartInfo(helper)
            {
                ArgumentList = { "--list" },
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
            });
            if (process is null) return outputs;

            var stdout = process.StandardOutput.ReadToEnd();
            process.StandardError.ReadToEnd();
            if (!process.WaitForExit(5_000)) { process.Kill(); return outputs; }

            foreach (var line in stdout.Split('\n', StringSplitOptions.RemoveEmptyEntries))
            {
                var parts = line.Split(' ', StringSplitOptions.RemoveEmptyEntries);
                if (parts.Length == 3 && int.TryParse(parts[1], out var w) && int.TryParse(parts[2], out var h))
                    outputs[parts[0]] = (w, h);
            }
        }
        catch (Exception)
        {
        }

        return outputs;
    }

    public static Process? Show(string output, string pngPath)
    {
        var helper = FindHelper();
        if (helper is null) return null;

        try
        {
            return Process.Start(new ProcessStartInfo(helper)
            {
                ArgumentList = { "--output", output, "--png", pngPath },
                UseShellExecute = false,
            });
        }
        catch (Exception)
        {
            return null;
        }
    }

    static string? FindHelper()
    {
        if (_searched) return _helper;
        _searched = true;

        var uiDir = AppContext.BaseDirectory.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);

        // 1. Deployed build: the helper sits next to the UI (staged like lbm-hook).
        var sibling = Path.Combine(uiDir, HelperName);
        if (File.Exists(sibling)) return _helper = sibling;

        // 2. Dev tree: the Rust build output (same convention as FindHookPath).
        try
        {
            var projectSegment = Path.Combine("LittleBigMouse.Ui", "LittleBigMouse.Ui.Avalonia");
            var i = uiDir.IndexOf(projectSegment, StringComparison.OrdinalIgnoreCase);
            if (i < 0) return null;
            var root = uiDir[..i];

            var sep = Path.DirectorySeparatorChar;
            var config = uiDir.Contains($"{sep}Debug{sep}", StringComparison.OrdinalIgnoreCase) ? "debug" : "release";

            var target = Path.Combine(root, "LittleBigMouse-Hook-Rust", "target");
            var candidates = new[]
            {
                Path.Combine(target, config, HelperName),
                Path.Combine(target, "release", HelperName),
                Path.Combine(target, "debug", HelperName),
            };
            return _helper = candidates.FirstOrDefault(File.Exists);
        }
        catch
        {
            return null;
        }
    }
}
