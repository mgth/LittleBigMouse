#nullable enable
using HLab.Geo;

namespace LittleBigMouse.DisplayLayout.Monitors.Extensions;

/// <summary>
/// The one place the reactive model is read into the plain records the solvers work on. Both
/// placement directions come through here, so "what a snapshot of a monitor is" is stated once:
/// the panel and bezel rects of its depth projection, and the pixel rect of its active source.
/// </summary>
public static class MonitorSnapshotExtensions
{
    /// <summary>
    /// Snapshot <paramref name="monitor"/>. The caller decides which monitor is primary — the
    /// layout's <c>PrimaryMonitor</c> and the source's <c>Primary</c> flag can disagree while a
    /// layout is being rebuilt, and each direction anchors on the one it was given.
    /// </summary>
    /// <param name="pixelSize">
    /// Substitute for the source's own pixel size, for the Wayland path that recomputes scales:
    /// the monitor is to be positioned at the size it will have after the change, not the one it
    /// has now.
    /// </param>
    public static MonitorSnapshot Snapshot(this PhysicalMonitor monitor, bool primary, Size? pixelSize = null)
    {
        var pixel = monitor.ActiveSource.Source.InPixel.Bounds;

        return new MonitorSnapshot(
            monitor.Id,
            monitor.DepthProjection.Bounds,
            monitor.DepthProjection.OutsideBounds,
            pixelSize is { } size ? new Rect(pixel.Location, size) : pixel,
            primary);
    }
}
