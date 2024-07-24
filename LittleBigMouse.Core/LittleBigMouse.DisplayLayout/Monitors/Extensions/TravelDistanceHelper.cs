using System.Collections.Generic;
using System.Linq;

namespace LittleBigMouse.DisplayLayout.Monitors.Extensions;

public static class TravelDistanceHelper
{
    class MonitorDistance
    {
        public required PhysicalMonitor Source;
        public required PhysicalMonitor Target;
        public double Distance;
    }

    public static double GetMinimalMaxTravelDistance(this IMonitorsLayout layout)
    {
        List<MonitorDistance> distances = [];

        var primary = layout.PrimaryMonitor;
        if (primary == null) return 0.0;

        foreach (var monitor in layout.PhysicalMonitors)
        {
            var distance = primary.DepthProjection.Bounds.Distance(monitor.DepthProjection.Bounds).DistanceHV();
            distances.Add(new MonitorDistance
            {
                Source = primary,
                Target = monitor,
                Distance = distance,
            });
        }

        var progress = true;

        while (progress)
        {
            distances = [.. distances.OrderBy(d => d.Distance)];
            var last = distances.Last();

            var others = distances.Except([last]).ToList();

            progress = false;

            foreach (var monitorDistance in others)
            {
                //check if monitor already in chain
                var d = monitorDistance;
                while(!ReferenceEquals(d.Source, primary))
                {
                    if(ReferenceEquals(d.Target,last.Target)) break;
                    d = distances.First(e => ReferenceEquals(e.Target, d.Source));
                }
                if(ReferenceEquals(d.Target,last.Target)) continue;


                var distance = last.Target.DepthProjection.Bounds.Distance(monitorDistance.Target.DepthProjection.Bounds).DistanceHV();
                if (distance >= last.Distance) continue;
                last.Source = monitorDistance.Target;
                last.Distance = distance;
                progress = true;
            }

        }
        return distances.Max(d => d.Distance);
    }
}