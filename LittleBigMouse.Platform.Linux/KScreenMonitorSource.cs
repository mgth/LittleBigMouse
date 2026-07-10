#nullable enable
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text.Json;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// KDE output enumeration through <c>kscreen-doctor --json</c> — works on both Wayland and
/// X11 Plasma sessions and is the only source that gives the compositor's own view (logical
/// positions, per-output scale, priority). Chosen over the org.kde.KScreen DBus API to keep
/// the dependency surface at zero; the DBus configChanged signal is the later event-driven
/// upgrade for change notification.
/// </summary>
public class KScreenMonitorSource : ILinuxMonitorSource
{
    public string Name => "kscreen";

    public bool IsAvailable()
        => Environment.GetEnvironmentVariable("XDG_CURRENT_DESKTOP")?.Contains("KDE", StringComparison.OrdinalIgnoreCase) == true
           && RunKScreenDoctor() != null;

    public List<LinuxMonitor> Query()
    {
        var json = RunKScreenDoctor();
        if (json == null) return [];

        var edids = DrmEdidReader.ReadAll();
        var monitors = new List<LinuxMonitor>();

        using var doc = JsonDocument.Parse(json);
        if (!doc.RootElement.TryGetProperty("outputs", out var outputs)) return monitors;

        foreach (var output in outputs.EnumerateArray())
        {
            if (!GetBool(output, "connected")) continue;

            var connector = GetString(output, "name") ?? "";
            var enabled = GetBool(output, "enabled");
            var scale = GetDouble(output, "scale", 1.0);
            if (scale <= 0) scale = 1.0;

            // KScreen rotation is a flag: 1=none, 2=90°, 4=180°, 8=270°.
            var orientation = GetInt(output, "rotation", 1) switch { 2 => 1, 4 => 2, 8 => 3, _ => 0 };

            var (modeW, modeH, frequency) = CurrentMode(output);
            if (orientation % 2 != 0) (modeW, modeH) = (modeH, modeW);

            double x = 0, y = 0;
            if (output.TryGetProperty("pos", out var pos))
            {
                x = GetDouble(pos, "x", 0);
                y = GetDouble(pos, "y", 0);
            }

            double widthMm = 0, heightMm = 0;
            if (output.TryGetProperty("sizeMM", out var mm))
            {
                widthMm = GetDouble(mm, "width", 0);
                heightMm = GetDouble(mm, "height", 0);
            }
            if (orientation % 2 != 0) (widthMm, heightMm) = (heightMm, widthMm);

            edids.TryGetValue(connector, out var edid);

            monitors.Add(new LinuxMonitor
            {
                ConnectorName = connector,
                LogicalX = x,
                LogicalY = y,
                LogicalWidth = Math.Round(modeW / scale),
                LogicalHeight = Math.Round(modeH / scale),
                PixelWidth = modeW,
                PixelHeight = modeH,
                Scale = scale,
                WidthMm = widthMm,
                HeightMm = heightMm,
                // Plasma 6 replaced the primary flag by a priority order: 1 is the primary,
                // 0/-1 means none (disabled outputs report -1).
                Primary = GetInt(output, "priority", 0) == 1,
                Enabled = enabled,
                Orientation = orientation,
                Frequency = frequency,
                Edid = edid
            });
        }

        // No priority-1 output (older Plasma, odd configs): fall back to the enabled output
        // at the logical origin, the model needs a primary to anchor on.
        if (monitors.Count > 0 && !monitors.Any(m => m.Primary))
        {
            var primary = monitors.FirstOrDefault(m => m is { Enabled: true, LogicalX: 0, LogicalY: 0 })
                          ?? monitors.First(m => m.Enabled);
            monitors[monitors.IndexOf(primary)] = primary with { Primary = true };
        }

        return monitors;
    }

    static (int Width, int Height, int Frequency) CurrentMode(JsonElement output)
    {
        var currentId = GetString(output, "currentModeId");
        if (currentId != null && output.TryGetProperty("modes", out var modes))
        {
            foreach (var mode in modes.EnumerateArray())
            {
                if (GetString(mode, "id") != currentId) continue;
                var frequency = (int)Math.Round(GetDouble(mode, "refreshRate", 0));
                if (mode.TryGetProperty("size", out var size))
                    return (GetInt(size, "width", 0), GetInt(size, "height", 0), frequency);
            }
        }

        // Disabled outputs may carry no current mode: fall back to the output size.
        if (output.TryGetProperty("size", out var s))
            return (GetInt(s, "width", 0), GetInt(s, "height", 0), 0);

        return (0, 0, 0);
    }

    internal static string? RunKScreenDoctor()
    {
        try
        {
            using var process = Process.Start(new ProcessStartInfo("kscreen-doctor", "--json")
            {
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false
            });
            if (process == null) return null;
            var stdout = process.StandardOutput.ReadToEnd();
            process.WaitForExit(5000);
            return process.ExitCode == 0 && stdout.TrimStart().StartsWith('{') ? stdout : null;
        }
        catch
        {
            return null;
        }
    }

    // Tolerant accessors: the kscreen-doctor schema moved between Plasma 5 and 6 (primary vs
    // priority, number formats) — a missing or oddly-typed field must never kill discovery.
    static string? GetString(JsonElement e, string name)
        => e.TryGetProperty(name, out var v)
            ? v.ValueKind switch
            {
                JsonValueKind.String => v.GetString(),
                JsonValueKind.Number => v.GetRawText(),
                _ => null
            }
            : null;

    static bool GetBool(JsonElement e, string name)
        => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.True;

    static double GetDouble(JsonElement e, string name, double fallback)
        => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number ? v.GetDouble() : fallback;

    static int GetInt(JsonElement e, string name, int fallback)
        => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number ? (int)Math.Round(v.GetDouble()) : fallback;
}
