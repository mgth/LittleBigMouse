using Avalonia;
using Avalonia.Controls;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

namespace LittleBigMouse.DisplayLayout.Monitors
{
    public static class MonitorExtensions
    {
        public static readonly Thickness _infinity = new(double.PositiveInfinity);

        public static bool IsPositiveInfinity(this Thickness @this)
        {
            return @this.Left == double.PositiveInfinity
                && @this.Top == double.PositiveInfinity
                && @this.Right == double.PositiveInfinity
                && @this.Bottom == double.PositiveInfinity;
        }

        public static Thickness DistanceToTouch(this PhysicalMonitor @this, PhysicalMonitor other, bool zero = false)
        {
            var distance = @this.Distance(other);
            if(distance.Top > 0 || distance.Bottom > 0 || zero && (distance.Top == 0 || distance.Bottom == 0))
            {
                if(distance.Left > 0 || distance.Right > 0 || zero && (distance.Left == 0 || distance.Right == 0))
                {
                    return _infinity;
                }
                return new Thickness(
                    double.PositiveInfinity,
                    distance.Top,
                    double.PositiveInfinity,
                    distance.Bottom
                );
            }
            if(distance.Left > 0 || distance.Right > 0 || zero && (distance.Left == 0 || distance.Right == 0))
            {
                return new Thickness(
                    distance.Left,
                    double.PositiveInfinity,
                    distance.Right,
                    double.PositiveInfinity
                );
            }

            return distance;
        }


        public static Thickness Distance(this PhysicalMonitor @this, PhysicalMonitor other)
        {
            return new Thickness(
                    other.DepthProjection.OutsideBounds.X - @this.DepthProjection.OutsideBounds.Right,
                    other.DepthProjection.OutsideBounds.Y - @this.DepthProjection.OutsideBounds.Bottom,
                    @this.DepthProjection.OutsideBounds.X - other.DepthProjection.OutsideBounds.Right,
                    @this.DepthProjection.OutsideBounds.Y - other.DepthProjection.OutsideBounds.Bottom
                );
        }

        public static Thickness Min(this Thickness @this, Thickness other)
        {
            return new Thickness(
                Math.Min(@this.Left, other.Left),
                Math.Min(@this.Top, other.Top),
                Math.Min(@this.Right, other.Right),
                Math.Min(@this.Bottom, other.Bottom)
           );
        }

        public static Thickness Distance(this PhysicalMonitor @this, IEnumerable<PhysicalMonitor> others)
        {
            var min = new Thickness(double.MaxValue);

            foreach (var other in others)
            {
                min = min.Min(@this.Distance(other));
            }
            return min;
        }
        public static Thickness DistanceToTouch(this PhysicalMonitor @this, IEnumerable<PhysicalMonitor> others, bool zero = false)
        {
            var min = new Thickness(double.MaxValue);

            foreach (var other in others)
            {
                min = min.Min(@this.DistanceToTouch(other, zero));
            }
            return min;
        }
    }
}
