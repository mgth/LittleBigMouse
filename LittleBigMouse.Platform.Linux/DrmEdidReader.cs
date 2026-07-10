#nullable enable
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using HLab.Sys.Monitors;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Reads monitor EDIDs from /sys/class/drm/card*-&lt;connector&gt;/edid and indexes them by
/// connector name (the "cardN-" prefix stripped, e.g. "DP-2"), which is how KScreen and
/// native-X11 XRandR name their outputs. On a multi-GPU machine the same connector name can
/// exist on several cards: only entries whose status is "connected" with a non-empty EDID
/// win, so the inactive card's dangling connectors never shadow the live ones.
/// </summary>
public static class DrmEdidReader
{
    public static Dictionary<string, Edid> ReadAll()
    {
        var result = new Dictionary<string, Edid>(StringComparer.OrdinalIgnoreCase);

        IEnumerable<string> connectors;
        try
        {
            connectors = Directory.EnumerateDirectories("/sys/class/drm", "card*-*");
        }
        catch
        {
            return result;
        }

        foreach (var dir in connectors)
        {
            try
            {
                if (File.ReadAllText(Path.Combine(dir, "status")).Trim() != "connected") continue;

                var edidBytes = File.ReadAllBytes(Path.Combine(dir, "edid"));
                if (edidBytes.Length < 128) continue;

                // "card1-DP-2" -> "DP-2"
                var name = Path.GetFileName(dir);
                var dash = name.IndexOf('-');
                if (dash < 0) continue;
                var connector = name[(dash + 1)..];

                result[connector] = EdidParser.Parse(dir, edidBytes);
            }
            catch
            {
                // unreadable connector entry: skip it, discovery must never fail here
            }
        }

        return result;
    }

    /// <summary>
    /// A cheap fingerprint of what is physically plugged: connector names, their status and
    /// EDID length. Pure sysfs reads — pollable at will. Catches plug/unplug and monitor
    /// swaps; compositor-side moves are only caught by the full <c>DisplaySignature</c>.
    /// </summary>
    public static string PlugSignature()
    {
        try
        {
            return string.Join("|",
                Directory.EnumerateDirectories("/sys/class/drm", "card*-*")
                    .OrderBy(d => d, StringComparer.Ordinal)
                    .Select(dir =>
                    {
                        string status;
                        try { status = File.ReadAllText(Path.Combine(dir, "status")).Trim(); }
                        catch { status = "?"; }

                        long edid = 0;
                        if (status == "connected")
                            try { edid = File.ReadAllBytes(Path.Combine(dir, "edid")).Length; }
                            catch { }

                        return $"{Path.GetFileName(dir)}:{status}:{edid}";
                    }));
        }
        catch
        {
            return "";
        }
    }
}
