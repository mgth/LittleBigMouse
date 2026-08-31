#nullable enable
using System;
using HLab.Sys.Monitors;
using HLab.Sys.Windows.Monitors;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Infers the panel's intrinsic physical size (mm) and its effective orientation from a
/// Win32 <see cref="MonitorDevice"/> snapshot. Pure functions over the Win32 tree — no OS
/// calls, no reactive model — so the tricky EDID/GDI/DPI fallback chain can be exercised
/// from unit tests with hand-built snapshots (incomplete EDID, virtual displays, rotation).
/// </summary>
internal static class WindowsPhysicalSize
{
    /// <summary>
    /// DEVMODE's orientation — unless the driver rotated below Windows (e.g. NVIDIA panel
    /// rotation) and left it at Default while the pixel mode is already transposed (#507).
    /// The panel's EDID aspect cannot rotate: when it contradicts the pixel aspect the
    /// display is effectively rotated, so report 90° and let the physical geometry
    /// transpose. Square or invalid sizes (EDID-less virtual displays report 0x0, #419)
    /// decide nothing.
    /// </summary>
    public static int InferOrientation(DisplayMode mode, Edid? edid)
    {
        if (mode.DisplayOrientation != 0) return mode.DisplayOrientation;

        if (edid == null) return 0;
        if (mode.Pels.Width == mode.Pels.Height) return 0;
        if (edid.PhysicalWidth <= 0 || edid.PhysicalHeight <= 0
            || edid.PhysicalWidth == edid.PhysicalHeight) return 0;

        var pixelPortrait = mode.Pels.Height > mode.Pels.Width;
        var panelPortrait = edid.PhysicalHeight > edid.PhysicalWidth;
        return pixelPortrait == panelPortrait ? 0 : 1;
    }

    /// <summary>
    /// Intrinsic physical size of the panel in millimeters — NEVER transposed to the current
    /// orientation. The model is shared by every monitor of the same PnP model whatever its
    /// rotation, and the DepthProjection/PhysicalRotated chain applies the rotation
    /// downstream: an oriented size here gets transposed twice, so a portrait display was
    /// placed with a landscape-looking geometry (#507).
    /// Windows GDI (HORZSIZE/VERTSIZE) stays the primary numeric source (TV EDIDs commonly
    /// lie about their size, and existing stored models were built from GDI), but it is
    /// normalized to the intrinsic orientation: drivers disagree on whether it follows the
    /// rotation, so both orientations are tested against an intrinsic aspect reference —
    /// the EDID aspect when present (an EDID never rotates, even when the rotation is done
    /// below Windows by the driver), the DEVMODE-unrotated resolution otherwise. A display
    /// without EDID (virtual, DisplayLink, RDP, spacedesk...) reports a bogus square
    /// placeholder (e.g. 1000x1000) that fails both tests: fall back to the EDID size, then
    /// to an estimate derived from the resolution and the DPI (HORZRES / LOGPIXELSX).
    /// </summary>
    public static (double Width, double Height) GetPhysicalSizeInMm(MonitorDevice monitor)
    {
        var edid = monitor.Edid is { PhysicalWidth: > 0, PhysicalHeight: > 0 } e ? monitor.Edid : null;

        var display = monitor.ActiveConnection?.Parent;

        if (display?.CurrentMode != null)
        {
            var caps = display.Capabilities;
            var rotated = display.CurrentMode.DisplayOrientation % 2 != 0;

            // Intrinsic (panel) resolution: HORZRES/VERTRES follow the current mode.
            var resW = rotated ? caps.Resolution.Height : caps.Resolution.Width;
            var resH = rotated ? caps.Resolution.Width : caps.Resolution.Height;

            // Intrinsic aspect reference (only the ASPECT is used, so mm vs px is fine).
            var (refW, refH) = edid != null ? (edid.PhysicalWidth, edid.PhysicalHeight) : (resW, resH);

            // GDI physical size, normalized to the intrinsic orientation.
            if (IsAspectConsistent(caps.Size.Width, caps.Size.Height, refW, refH))
                return (caps.Size.Width, caps.Size.Height);
            if (IsAspectConsistent(caps.Size.Height, caps.Size.Width, refW, refH))
                return (caps.Size.Height, caps.Size.Width);

            // GDI size unreliable (EDID-less display): prefer the EDID size when available.
            if (edid != null)
                return (edid.PhysicalWidth, edid.PhysicalHeight);

            // Otherwise estimate from the resolution and the DPI: inches = pixels / dpi.
            var dpiW = rotated ? caps.LogPixels.Height : caps.LogPixels.Width;
            var dpiH = rotated ? caps.LogPixels.Width : caps.LogPixels.Height;
            if (dpiW > 0 && dpiH > 0)
                return (resW / dpiW * 25.4, resH / dpiH * 25.4);

            return (caps.Size.Width, caps.Size.Height); // nothing better than the GDI value
        }

        // Detached / no current mode: rely on EDID if present.
        if (edid != null)
            return (edid.PhysicalWidth, edid.PhysicalHeight);

        return (0, 0);
    }

    /// <summary>
    /// True when the physical size aspect ratio roughly matches the pixel aspect ratio
    /// (square pixels). A square placeholder (1000x1000) against a 16:9 resolution fails this.
    /// </summary>
    public static bool IsAspectConsistent(double width, double height, double pixelsWidth, double pixelsHeight)
    {
        if (width <= 0 || height <= 0 || pixelsWidth <= 0 || pixelsHeight <= 0) return false;

        var sizeAspect = width / height;
        var pixelAspect = pixelsWidth / pixelsHeight;

        return Math.Abs(sizeAspect / pixelAspect - 1.0) < 0.12;
    }
}
