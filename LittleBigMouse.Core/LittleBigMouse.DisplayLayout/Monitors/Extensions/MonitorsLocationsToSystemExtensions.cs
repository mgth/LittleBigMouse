#nullable enable
using System;
using System.Collections.Generic;
using System.Linq;
using HLab.Geo;

namespace LittleBigMouse.DisplayLayout.Monitors.Extensions;

/// <summary>Result of <see cref="MonitorsLocationsToSystemExtensions.ComputePixelLocationsFromPhysical"/> for one source.</summary>
public sealed record SystemPlacement(Rect PixelBounds, double? Scale);

public static class MonitorsLocationsToSystemExtensions
{
    /// <summary>
    /// Compute the system pixel configuration matching the physical layout. With
    /// <paramref name="adjustScale"/> (Wayland only) per-output scales are recomputed so a
    /// logical pixel covers the same physical size everywhere (the primary's current logical
    /// pitch), quantized to 1/120 — the fractional-scale protocol unit KWin rounds to.
    /// Scale is null when unchanged; Windows callers ignore it entirely.
    /// <para>
    /// The geometry lives in <see cref="PixelLocationSolver"/>; this method snapshots the
    /// reactive model, works out the pixel size each monitor will have after the change, solves,
    /// and hands the result back keyed by source.
    /// </para>
    /// </summary>
    public static Dictionary<DisplaySource, SystemPlacement> ComputePixelLocationsFromPhysical(
        this IMonitorsLayout layout, bool adjustScale = false)
    {
        var result = new Dictionary<DisplaySource, SystemPlacement>();

        var primary = layout.PrimaryMonitor;
        if (primary?.ActiveSource == null) return result;

        var monitors = layout.PhysicalMonitors
            .Where(m => m.ActiveSource?.Source.AttachedToDesktop == true)
            .ToList();
        if (monitors.Count == 0) return result;

        var inputs = new List<MonitorSnapshot>();
        var sizes = new Dictionary<string, (DisplaySource Source, Size PixelSize, double? Scale)>();

        // The primary's logical pitch (mm per logical pixel) is the homogeneity target:
        // its own scale never changes, everything else converges to it.
        var targetPitch = primary.DepthProjection.Bounds.Width / primary.ActiveSource.Source.InPixel.Width;

        foreach (var monitor in monitors)
        {
            var source = monitor.ActiveSource.Source;
            var pixelSize = source.InPixel.Bounds.Size;
            double? newScale = null;

            if (adjustScale)
            {
                var scale = source.EffectiveDpi.X / 96;
                var native = new Size(source.InPixel.Width * scale, source.InPixel.Height * scale);
                var physicalPitch = monitor.DepthProjection.Bounds.Width / native.Width;

                var wanted = Math.Round(targetPitch / physicalPitch * 120) / 120;
                wanted = Math.Clamp(wanted, 0.5, 3.0);

                // 1/240 = half a protocol step: below that the quantized scale is the
                // one the compositor already runs, don't emit a no-op change.
                if (Math.Abs(wanted - scale) >= 1.0 / 240)
                {
                    newScale = wanted;
                    pixelSize = new Size(Math.Round(native.Width / wanted), Math.Round(native.Height / wanted));
                }
            }

            inputs.Add(monitor.Snapshot(ReferenceEquals(monitor, primary), pixelSize));
            sizes[monitor.Id] = (source, pixelSize, newScale);
        }

        var solved = PixelLocationSolver.Solve(inputs);

        foreach (var (id, position) in solved)
        {
            var (source, pixelSize, scale) = sizes[id];
            result[source] = new SystemPlacement(new Rect(position, pixelSize), scale);
        }

        return result;
    }
}
