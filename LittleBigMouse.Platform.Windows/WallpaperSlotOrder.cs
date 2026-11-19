#nullable enable
using System;
using System.Collections.Generic;
using System.Linq;
using HLab.Geo;
using HLab.Sys.Windows.API;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// The two monitor orders behind the shell's per-monitor wallpaper.
/// <para>
/// The shell files an image under the slot of the device path it is written through — the
/// <c>GetMonitorDevicePathAt</c> order, which is also what <c>TranscodedImageCache_00k</c>
/// records — but it paints slot <c>k</c> on the k-th monitor of a different order: the primary
/// first, then the others. On a layout whose primary is not the first enumerated monitor (a
/// portrait screen on DP at the left, the landscape primary on HDMI) the two orders differ,
/// every image lands one screen off and the desktop shows the screens swapped.
/// </para>
/// <para>
/// Both ends have to use the same pairing: the writer, to file each image under the slot that
/// will paint its screen; the reader, to attribute a slot's image to the screen it is painted
/// on instead of to the screen its path names.
/// </para>
/// </summary>
internal static class WallpaperSlotOrder
{
    /// <summary>One filled slot: the path it is filed under, that monitor's rect, its image.</summary>
    internal readonly record struct Slot(string DevicePath, WinDef.Rect Rect, string ImagePath);

    /// <summary>
    /// Reads the slots in filing order. Rectangles that are empty (a monitor that was
    /// unplugged but is still listed, or a stale entry) are dropped, as they match no screen.
    /// </summary>
    internal static bool TryRead(IDesktopWallpaper desktop, out List<Slot> slots)
    {
        slots = [];

        uint count = 0;
        if (desktop.GetMonitorDevicePathCount(ref count) != HResult.Ok) return false;

        for (uint i = 0; i < count; i++)
        {
            var devicePath = "";
            var imagePath = "";
            WinDef.Rect rect = new();

            if (desktop.GetMonitorDevicePathAt(i, ref devicePath) != HResult.Ok) continue;
            if (desktop.GetMonitorRECT(devicePath, ref rect) != HResult.Ok) continue;
            if (rect.Right <= rect.Left || rect.Bottom <= rect.Top) continue;
            if (desktop.GetWallpaper(devicePath, ref imagePath) != HResult.Ok) imagePath = "";

            slots.Add(new Slot(devicePath, rect, imagePath));
        }

        return slots.Count > 0;
    }

    /// <summary>
    /// Pairs every filed slot with the rectangle the shell paints it on: the primary first, the
    /// rest in filing order. Identity on the layouts where both orders already agree.
    /// </summary>
    internal static List<(Slot Filed, WinDef.Rect Painted)> Pair(IReadOnlyList<Slot> slots)
    {
        // The primary holds the origin of the virtual desktop; stable sort keeps the others
        // in filing order.
        var painted = slots
            .OrderBy(s => s.Rect.X == 0 && s.Rect.Y == 0 ? 0 : 1)
            .ToList();

        var pairs = new List<(Slot Filed, WinDef.Rect Painted)>(slots.Count);
        for (var i = 0; i < slots.Count; i++)
            pairs.Add((slots[i], painted[i].Rect));

        return pairs;
    }

    /// <summary>True when the layout bounds cover that monitor rect (integer rounding tolerant).</summary>
    internal static bool Covers(WinDef.Rect rect, Rect bounds)
        => Math.Abs(rect.X - bounds.X) < 2 && Math.Abs(rect.Y - bounds.Y) < 2
           && Math.Abs(rect.Width - bounds.Width) < 2 && Math.Abs(rect.Height - bounds.Height) < 2;
}
