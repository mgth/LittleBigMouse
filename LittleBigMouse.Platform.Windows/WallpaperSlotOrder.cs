#nullable enable
using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading;
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
/// <para>
/// The pairing is positional, so a slot that could not be read must keep its position: dropping
/// it would shift every later slot by one and send an image to a monitor nobody configured. A
/// slot whose lookup failed is therefore kept as a hole, and a pair that touches a hole is left
/// out of <see cref="SlotPlan.Pairs"/> — its destination is uncertain.
/// </para>
/// </summary>
internal static class WallpaperSlotOrder
{
    /// <summary>One readable slot: the path it is filed under, that monitor's rect, its image.</summary>
    /// <param name="ImagePath">
    /// What <c>GetWallpaper</c> returned for this slot: "" for a solid colour (or the fade
    /// transient), <c>null</c> when the call itself failed.
    /// </param>
    internal readonly record struct Slot(string DevicePath, WinDef.Rect Rect, string? ImagePath);

    /// <summary>
    /// The raw read of one enumerated slot, failures included: <see cref="DevicePath"/> is null
    /// when <c>GetMonitorDevicePathAt</c> failed, <see cref="Rect"/> when <c>GetMonitorRECT</c>
    /// did, <see cref="ImagePath"/> when <c>GetWallpaper</c> did.
    /// </summary>
    internal readonly record struct SlotRead(string? DevicePath, WinDef.Rect? Rect, string? ImagePath)
    {
        /// <summary>A lookup failed: the slot may be an active monitor we cannot place.</summary>
        public bool Unknown => DevicePath is null || Rect is null;

        /// <summary>Read fine, with an empty rectangle: listed but not on the desktop.</summary>
        public bool Inactive => !Unknown && IsEmpty(Rect!.Value);
    }

    /// <summary>
    /// The pairing of one snapshot. <see cref="Pairs"/> holds only the certain pairs: both the
    /// filing slot and the painted slot were read. <see cref="Slots"/> is every readable active
    /// slot, the set of paths an independent write must not collide with.
    /// <see cref="Complete"/> is false when some active slot could not be read.
    /// </summary>
    internal sealed record SlotPlan(
        IReadOnlyList<(Slot Filed, Slot Painted)> Pairs,
        IReadOnlyList<Slot> Slots,
        bool Complete)
    {
        /// <summary>True when a write through <paramref name="devicePath"/> would land in a slot of this plan.</summary>
        public bool Files(string devicePath)
            => Slots.Any(s => string.Equals(s.DevicePath, devicePath, StringComparison.OrdinalIgnoreCase));
    }

    /// <summary>
    /// Reads every enumerated slot in filing order, keeping failed lookups in place.
    /// Null when the shell cannot even count its monitors.
    /// </summary>
    internal static List<SlotRead>? Read(IDesktopWallpaper desktop)
    {
        uint count = 0;
        if (desktop.GetMonitorDevicePathCount(ref count) != HResult.Ok) return null;

        var reads = new List<SlotRead>((int)count);
        for (uint i = 0; i < count; i++)
        {
            var devicePath = "";
            if (desktop.GetMonitorDevicePathAt(i, ref devicePath) != HResult.Ok || devicePath is null)
            {
                reads.Add(new SlotRead(null, null, null));
                continue;
            }

            // S_FALSE is the documented answer for a monitor that is not attached: a stale
            // entry, read as an empty rectangle (Inactive). Only another failure may hide an
            // active monitor.
            WinDef.Rect rect = new();
            var found = desktop.GetMonitorRECT(devicePath, ref rect);
            if (found == HResult.False)
            {
                reads.Add(new SlotRead(devicePath, new WinDef.Rect(), null));
                continue;
            }
            if (found != HResult.Ok)
            {
                reads.Add(new SlotRead(devicePath, null, null));
                continue;
            }

            var imagePath = "";
            var image = desktop.GetWallpaper(devicePath, ref imagePath) == HResult.Ok ? imagePath ?? "" : null;

            reads.Add(new SlotRead(devicePath, rect, image));
        }

        return reads;
    }

    /// <summary>
    /// Pairs every filed slot with the slot the shell paints it on: the primary first, the rest
    /// in filing order. Identity on the layouts where both orders already agree.
    /// <para>
    /// <paramref name="activeMonitors"/> (the desktop monitor count, <c>SM_CMONITORS</c>) tells an
    /// unreadable slot that is an active monitor from a stale entry: when every active monitor
    /// has been read, the unreadable ones are not on the desktop and drop out; when exactly the
    /// unreadable ones are missing, they stay as holes. Any other count, or a hole that may be
    /// the primary while it is not the only hole, makes the snapshot unusable: null.
    /// </para>
    /// </summary>
    internal static SlotPlan? Plan(IReadOnlyList<SlotRead> reads, int activeMonitors)
    {
        var known = reads.Count(r => !r.Unknown && !r.Inactive);
        var unknown = reads.Count(r => r.Unknown);

        // Active monitors in filing order; null marks a hole.
        List<Slot?> active;
        if (unknown == 0 || known == activeMonitors)
        {
            active = reads.Where(r => !r.Unknown && !r.Inactive).Select(ToSlot).ToList();
        }
        else if (activeMonitors > 0 && known + unknown == activeMonitors)
        {
            active = reads.Where(r => !r.Inactive).Select(r => r.Unknown ? null : ToSlot(r)).ToList();
        }
        else
        {
            return null;
        }

        if (active.Count == 0) return null;

        // The primary holds the origin of the virtual desktop. If no readable slot does, the
        // primary is a hole — placeable only when there is a single one.
        var primary = active.FindIndex(s => s is { } slot && slot.Rect.X == 0 && slot.Rect.Y == 0);
        var holes = active.Count(s => s is null);
        // With no hole and no monitor at the origin, primary stays -1: filing order is kept.
        if (primary < 0 && holes == 1) primary = active.FindIndex(s => s is null);
        else if (primary < 0 && holes > 1) return null;

        var painted = new List<Slot?>(active.Count);
        if (primary >= 0) painted.Add(active[primary]);
        for (var i = 0; i < active.Count; i++)
            if (i != primary) painted.Add(active[i]);

        var pairs = new List<(Slot Filed, Slot Painted)>(active.Count);
        for (var i = 0; i < active.Count; i++)
            if (active[i] is { } filed && painted[i] is { } on)
                pairs.Add((filed, on));

        return new SlotPlan(pairs, active.OfType<Slot>().ToList(), holes == 0);

        static Slot? ToSlot(SlotRead r) => new Slot(r.DevicePath!, r.Rect!.Value, r.ImagePath);
    }

    /// <summary>
    /// Reads and plans, retrying a snapshot that has holes or does not add up: a lookup that
    /// fails transiently usually succeeds a moment later, and writing from a full snapshot
    /// beats skipping a screen. Returns the last plan (possibly incomplete) or null.
    /// <paramref name="enumerated"/> is false when the shell could not enumerate at all.
    /// </summary>
    internal static SlotPlan? ReadPlan(IDesktopWallpaper desktop, int activeMonitors, int attempts,
        out bool enumerated, TimeSpan? retryDelay = null)
    {
        SlotPlan? plan = null;
        enumerated = false;

        for (var attempt = 0; attempt < Math.Max(1, attempts); attempt++)
        {
            if (attempt > 0) Thread.Sleep(retryDelay ?? TimeSpan.FromMilliseconds(50));

            if (Read(desktop) is not { } reads) continue;
            enumerated = true;

            plan = Plan(reads, activeMonitors);
            if (plan is { Complete: true }) return plan;
        }

        return plan;
    }

    /// <summary>Number of monitors on the desktop (<c>SM_CMONITORS</c>), 0 off Windows or on failure.</summary>
    internal static int ActiveMonitorCount()
    {
        if (!OperatingSystem.IsWindows()) return 0;
        try
        {
            return GetSystemMetrics(SmCMonitors);
        }
        catch (Exception)
        {
            return 0;
        }
    }

    const int SmCMonitors = 80;

    [DllImport("user32.dll")]
    static extern int GetSystemMetrics(int index);

    static bool IsEmpty(WinDef.Rect rect) => rect.Right <= rect.Left || rect.Bottom <= rect.Top;

    /// <summary>True when the layout bounds cover that monitor rect (integer rounding tolerant).</summary>
    internal static bool Covers(WinDef.Rect rect, Rect bounds)
        => Math.Abs(rect.X - bounds.X) < 2 && Math.Abs(rect.Y - bounds.Y) < 2
           && Math.Abs(rect.Width - bounds.Width) < 2 && Math.Abs(rect.Height - bounds.Height) < 2;
}
