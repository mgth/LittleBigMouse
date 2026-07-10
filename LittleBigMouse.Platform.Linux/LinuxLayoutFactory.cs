#nullable enable
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text.RegularExpressions;
using DynamicData;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Linux implementation of <see cref="ILayoutFactory"/>. Phase 1: monitor discovery goes
/// through <c>xrandr --query</c> (X11 or XWayland). Under a Wayland session XWayland reports
/// synthetic outputs (XWAYLAND0…) with no connector identity, so EDID matching is impossible
/// here — phase 2 replaces this source with kscreen-doctor/DRM-sysfs on KDE and keeps xrandr
/// as the generic X11 fallback.
/// </summary>
public class LinuxLayoutFactory : ILayoutFactory
{
    readonly Func<MonitorsLayout> _newLayout;

    public LinuxLayoutFactory(Func<MonitorsLayout> newLayout)
    {
        _newLayout = newLayout;
    }

    // Wallpaper preview is not implemented on Linux yet: the editor renders a flat background.
    public event EventHandler? WallpaperChanged;

    public void UpdateWallpaper(MonitorsLayout layout) { }

    public MonitorsLayout Create()
    {
        var layout = _newLayout();

        var outputs = XRandROutput.Query();
        if (outputs.Count == 0)
        {
            // xrandr unavailable or no active output: still open the UI on a plausible
            // single monitor rather than an empty editor.
            outputs = [XRandROutput.Fallback()];
        }

        foreach (var output in outputs)
        {
            var model = layout.GetOrAddPhysicalMonitorModel(output.Name, s =>
            {
                var m = new PhysicalMonitorModel(s) { PnpDeviceName = output.Name };
                if (output is { WidthMm: > 0, HeightMm: > 0 })
                {
                    var fixedRatio = m.PhysicalSize.FixedAspectRatio;
                    m.PhysicalSize.FixedAspectRatio = false;
                    m.PhysicalSize.Width = output.WidthMm;
                    m.PhysicalSize.Height = output.HeightMm;
                    m.PhysicalSize.FixedAspectRatio = fixedRatio;
                }
                return m;
            });

            var monitor = new PhysicalMonitor(output.Name, layout, model)
            {
                DeviceId = output.Name,
                SerialNumber = "N/A"
            };

            var source = new DisplaySource(output.Name)
            {
                InterfacePath = output.Name,
                DeviceName = output.Name,
                DisplayName = output.Name,
                SourceName = output.Name,
                SourceNumber = (outputs.IndexOf(output) + 1).ToString(),
                Primary = output.Primary,
                AttachedToDesktop = true,
                Orientation = output.Orientation,
                DisplayFrequency = 0
            };
            source.InPixel.Set(new HLab.Geo.Rect(
                new HLab.Geo.Point(output.X, output.Y),
                new HLab.Geo.Size(output.Width, output.Height)));

            // xrandr has no DPI notion per output: derive it from pixels vs millimeters.
            var dpiX = output.WidthMm > 0 ? output.Width * 25.4 / output.WidthMm : 96;
            var dpiY = output.HeightMm > 0 ? output.Height * 25.4 / output.HeightMm : 96;
            source.EffectiveDpi.Set(dpiX, dpiY);
            source.RawDpi.Set(dpiX, dpiY);
            source.DpiAwareAngularDpi.Set(dpiX, dpiY);

            var physicalSource = new PhysicalSource(output.Name, monitor, source);
            monitor.ActiveSource = physicalSource;
            monitor.Sources.Add(physicalSource);

            layout.AddOrUpdatePhysicalMonitor(monitor);
            layout.AddOrUpdatePhysicalSource(physicalSource);
        }

        layout.Id = string.Join("+", layout.PhysicalMonitors.Select(m => $"{m.Id}").OrderBy(s => s));

        layout.SetLocationsFromSystemConfiguration();
        layout.AnchorOnPrimary();

        return layout;
    }

    public string DisplaySignature()
        => string.Join("|", XRandROutput.Query()
            .Select(o => $"{o.Name}[{o.X},{o.Y} {o.Width}x{o.Height}]{(o.Primary ? "*" : "")}d{o.WidthMm}x{o.HeightMm}"));
}

/// <summary>One active xrandr output (parsed from <c>xrandr --query</c>).</summary>
public record XRandROutput(string Name, int X, int Y, int Width, int Height, int WidthMm, int HeightMm, bool Primary, int Orientation)
{
    // "DP-4 connected primary 3840x2160+0+0 left (normal left inverted right x axis y axis) 597mm x 336mm"
    static readonly Regex Line = new(
        @"^(?<name>\S+) connected(?<primary> primary)? (?<w>\d+)x(?<h>\d+)\+(?<x>-?\d+)\+(?<y>-?\d+)(?<rot> normal| left| inverted| right)?(?: \([^)]*\))?(?: (?<wmm>\d+)mm x (?<hmm>\d+)mm)?",
        RegexOptions.Compiled);

    public static XRandROutput Fallback() => new("FALLBACK", 0, 0, 1920, 1080, 527, 296, true, 0);

    public static List<XRandROutput> Query()
    {
        string stdout;
        try
        {
            using var process = Process.Start(new ProcessStartInfo("xrandr", "--query")
            {
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false
            });
            if (process == null) return [];
            stdout = process.StandardOutput.ReadToEnd();
            process.WaitForExit(5000);
            if (process.ExitCode != 0) return [];
        }
        catch
        {
            return [];
        }

        var outputs = new List<XRandROutput>();
        foreach (var line in stdout.Split('\n'))
        {
            var match = Line.Match(line);
            if (!match.Success) continue;

            var orientation = match.Groups["rot"].Value.Trim() switch
            {
                "left" => 1,
                "inverted" => 2,
                "right" => 3,
                _ => 0
            };

            outputs.Add(new XRandROutput(
                match.Groups["name"].Value,
                int.Parse(match.Groups["x"].Value),
                int.Parse(match.Groups["y"].Value),
                int.Parse(match.Groups["w"].Value),
                int.Parse(match.Groups["h"].Value),
                match.Groups["wmm"].Success ? int.Parse(match.Groups["wmm"].Value) : 0,
                match.Groups["hmm"].Success ? int.Parse(match.Groups["hmm"].Value) : 0,
                match.Groups["primary"].Success,
                orientation));
        }

        // xrandr marks no output as primary under some compositors: pick the one at (0,0),
        // the model needs a primary to anchor and place monitors.
        if (outputs.Count > 0 && !outputs.Any(o => o.Primary))
        {
            var first = outputs.FirstOrDefault(o => o is { X: 0, Y: 0 }) ?? outputs[0];
            outputs[outputs.IndexOf(first)] = first with { Primary = true };
        }

        return outputs;
    }
}
