#nullable enable
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text.RegularExpressions;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Generic X11 fallback through <c>xrandr --query</c>, for sessions without KScreen. On
/// native X11 the output names are real connectors (EDID matching in sysfs works) and the
/// geometry is in pixels — logical == pixels, scale 1. Under a Wayland compositor other
/// than KWin this sees whatever XWayland exposes, which is still enough to edit a layout.
/// </summary>
public class XRandRMonitorSource : ILinuxMonitorSource
{
    public string Name => "xrandr";

    // "DP-4 connected primary 3840x2160+0+0 left (normal left inverted right x axis y axis) 597mm x 336mm"
    static readonly Regex Line = new(
        @"^(?<name>\S+) connected(?<primary> primary)? (?<w>\d+)x(?<h>\d+)\+(?<x>-?\d+)\+(?<y>-?\d+)(?<rot> normal| left| inverted| right)?(?: \([^)]*\))?(?: (?<wmm>\d+)mm x (?<hmm>\d+)mm)?",
        RegexOptions.Compiled);

    public bool IsAvailable()
        => !string.IsNullOrEmpty(Environment.GetEnvironmentVariable("DISPLAY")) && RunXRandR() != null;

    public List<LinuxMonitor> Query()
    {
        var stdout = RunXRandR();
        if (stdout == null) return [];

        var edids = DrmEdidReader.ReadAll();
        var monitors = new List<LinuxMonitor>();

        foreach (var line in stdout.Split('\n'))
        {
            var match = Line.Match(line);
            if (!match.Success) continue;

            var connector = match.Groups["name"].Value;
            var orientation = match.Groups["rot"].Value.Trim() switch
            {
                "left" => 1,
                "inverted" => 2,
                "right" => 3,
                _ => 0
            };

            edids.TryGetValue(connector, out var edid);

            var width = int.Parse(match.Groups["w"].Value);
            var height = int.Parse(match.Groups["h"].Value);

            monitors.Add(new LinuxMonitor
            {
                ConnectorName = connector,
                LogicalX = int.Parse(match.Groups["x"].Value),
                LogicalY = int.Parse(match.Groups["y"].Value),
                LogicalWidth = width,
                LogicalHeight = height,
                PixelWidth = width,
                PixelHeight = height,
                WidthMm = match.Groups["wmm"].Success ? int.Parse(match.Groups["wmm"].Value) : 0,
                HeightMm = match.Groups["hmm"].Success ? int.Parse(match.Groups["hmm"].Value) : 0,
                Primary = match.Groups["primary"].Success,
                Orientation = orientation,
                Edid = edid
            });
        }

        // xrandr marks no output as primary under some compositors: pick the one at (0,0),
        // the model needs a primary to anchor and place monitors.
        if (monitors.Count > 0 && !monitors.Any(m => m.Primary))
        {
            var primary = monitors.FirstOrDefault(m => m is { LogicalX: 0, LogicalY: 0 }) ?? monitors[0];
            monitors[monitors.IndexOf(primary)] = primary with { Primary = true };
        }

        return monitors;
    }

    static string? RunXRandR()
    {
        try
        {
            using var process = Process.Start(new ProcessStartInfo("xrandr", "--query")
            {
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false
            });
            if (process == null) return null;
            var stdout = process.StandardOutput.ReadToEnd();
            process.WaitForExit(5000);
            return process.ExitCode == 0 ? stdout : null;
        }
        catch
        {
            return null;
        }
    }
}
