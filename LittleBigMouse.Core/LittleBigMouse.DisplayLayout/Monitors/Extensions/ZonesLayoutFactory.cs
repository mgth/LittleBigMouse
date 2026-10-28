using System;
using System.Linq;
using HLab.Geo;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.DisplayLayout.Monitors.Extensions;

public static class ZonesLayoutFactory
{
    public static ZonesLayout ComputeZones(this IMonitorsLayout layout)
    {
        var zones = new ZonesLayout();
        foreach (var source in layout.PhysicalSources)
        {
            // Excluded monitors (pump LCDs, sensor panels…) get no zone: the cursor
            // treats them as a wall (#504). Never exclude the primary though — with
            // no zone under the cursor's home monitor the engine would go inert.
            if (source.Monitor.ExcludedFromLayout && !source.Source.Primary) continue;

            if (source == source.Monitor.ActiveSource && source.Source.AttachedToDesktop)
                zones.Zones.Add(new Zone(
                    source.Monitor.BorderResistance,
                    source.Source.Id,
                    source.Monitor.Model.PnpDeviceName,
                    source.Source.InPixel.Bounds,
                    source.Monitor.DepthProjection.Bounds
                ));
        }

        var actualZones = zones.Zones.ToArray();

        if (layout.Options.LoopX)
        {
            var shiftLeft = new Vector(-layout.PhysicalBounds.Width, 0);
            var shiftRight = new Vector(layout.PhysicalBounds.Width, 0);

            foreach (var zone in actualZones)
            {
                zones.Zones.Add(new(zone.BorderResistance, zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftLeft), zone));
                zones.Zones.Add(new(zone.BorderResistance, zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftRight), zone));
            }
        }

        if (layout.Options.LoopY)
        {
            var shiftUp = new Vector(0, -layout.PhysicalBounds.Height);
            var shiftDown = new Vector(0, layout.PhysicalBounds.Height);

            foreach (var zone in actualZones)
            {
                zones.Zones.Add(new(zone.BorderResistance, zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftUp), zone));
                zones.Zones.Add(new(zone.BorderResistance, zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftDown), zone));
            }
        }

        zones.MaxTravelDistance = layout.Options.MaxTravelDistance;
        zones.FreelookCheckInterval = layout.Options.FreelookCheckInterval;
        zones.FreelookEnabled = layout.Options.FreelookEnabled;

        zones.AdjustPointer = layout.Options.AdjustPointer;
        zones.AdjustSpeed = layout.Options.AdjustSpeed;

        zones.TouchMouseIndependent = layout.Options.TouchMouseIndependent;
        zones.StylusMouseIndependent = layout.Options.StylusMouseIndependent;
        zones.RestoreKeyboardFocus = layout.Options.RestoreKeyboardFocus;
        zones.FocusRestoreDelay = layout.Options.FocusRestoreDelay;
        zones.FocusRestoreOnMouseMove = layout.Options.FocusRestoreOnMouseMove;
        zones.TouchAllDisplays = layout.Options.TouchAllDisplays;
        zones.TouchDisplayIds = layout.Options.TouchDisplayIds;
        zones.TouchOverrideModifier = layout.Options.TouchOverrideModifier;
        zones.FocusKeepApps = layout.Options.FocusKeepApps.Replace("\r", "").Replace("\n", ";");
        zones.FocusRestoreApps = layout.Options.FocusRestoreApps.Replace("\r", "").Replace("\n", ";");
        // Separate from cursor-routing zones: sensor panels excluded from the layout
        // can still participate in touchscreen focus restoration.
        if (!layout.Options.TouchAllDisplays)
        {
            var selected = layout.Options.TouchDisplayIds
                .Split(';', StringSplitOptions.RemoveEmptyEntries)
                .ToHashSet(StringComparer.Ordinal);
            zones.TouchDisplayBounds = string.Join(";", layout.PhysicalSources
                .Where(s => s == s.Monitor.ActiveSource && s.Source.AttachedToDesktop
                    && selected.Contains(s.Source.Id))
                .Select(s =>
                {
                    var bounds = s.Source.InPixel.Bounds;
                    return FormattableString.Invariant(
                        $"{bounds.Left},{bounds.Top},{bounds.Width},{bounds.Height}");
                }));
        }

        zones.RescueShortcut = layout.Options.RescueShortcut;

        zones.Algorithm = layout.Options.Algorithm;
        zones.Priority = layout.Options.Priority;
        zones.PriorityUnhooked = layout.Options.PriorityUnhooked;

        zones.LoopX = layout.Options.LoopX;
        zones.LoopY = layout.Options.LoopY;

        // Carried on the wire: the daemon keys its "never hook a virtual layout"
        // guard on this flag, independently of what the UI sends afterwards.
        zones.Virtual = layout.IsVirtual;

        // Init() computes zone links via ComputeLinks(), which reads MaxTravelDistance:
        // it must run after the options above are set, otherwise links are built with
        // default values (cursor stuck at some edges, wrong travel distance).
        zones.Init();

        return zones;
    }

}
