using LittleBigMouse.Zoning;
using Avalonia;

namespace LittleBigMouse.DisplayLayout.Monitors.Extensions;

public static class ZonesLayoutFactory
{
    public static ZonesLayout ComputeZones(this IMonitorsLayout layout)
    {
        var zones = new ZonesLayout();
        foreach (var source in layout.PhysicalSources)
        {
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

        zones.Init();

        zones.MaxTravelDistance = layout.Options.MaxTravelDistance;

        zones.AdjustPointer = layout.Options.AdjustPointer;
        zones.AdjustSpeed = layout.Options.AdjustSpeed;

        zones.Algorithm = layout.Options.Algorithm;
        zones.Priority = layout.Options.Priority;
        zones.PriorityUnhooked = layout.Options.PriorityUnhooked;

        zones.LoopX = layout.Options.LoopX;
        zones.LoopY = layout.Options.LoopY;

        return zones;
    }

}