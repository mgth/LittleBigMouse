using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;

namespace LittleBigMouse.Zoning
{
    public static class RectExtentions
    {
        public static IEnumerable<Rect> TravelPixels(this Rect source, Rect target, IEnumerable<Rect> allowed)
        {
            (var a, var b) = source.Reachable(target);

            if (b.HasValue) return new Rect[]{a, b.Value};

            if(allowed.Any())
            {
                foreach(var next in allowed)
                {
                    var newAllowed = allowed.Where(r => r.Equals(next)).ToArray();

                    var tail = next.TravelPixels(target,newAllowed);
                    if(tail.Any())
                    {
                        //todo : should retain smallest travel 
                        var travel = source.TravelPixels(next,newAllowed);
                        if(travel.Any())
                        {
                            return travel.Concat(tail).ToArray();
                        }
                    }
                }
            }

            return Array.Empty<Rect>();
        }

        public static (Rect,Rect?) Reachable(this Rect source, Rect target)
        {
            var left   = Math.Max(target.Left, source.Left);
            var right  = Math.Min(target.Right, source.Right);
            var top    = Math.Max(target.Top, source.Top);
            var bottom = Math.Min(target.Bottom, source.Bottom);

            if(left >= right) 
            {
                if(top >= bottom)
                {
                    return (source,null);
                }
                else
                { 
                    var start = new Rect(source.Left, top, source.Width, bottom - top);
                    var dest  = new Rect(target.Left, top, target.Width, bottom - top);
                    return (start,dest);
                }
            }
            else if(top >= bottom) 
            {
                var start = new Rect(left, source.Top, right-left, source.Height);
                var dest  = new Rect(left, target.Top, right-left, target.Height);
                return (start,dest);
            }
            else
            {
                var start = new Rect(left, top, right-left, bottom - top);
                return (start,start);
            }
        }

    }
}
