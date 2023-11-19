using Avalonia;

namespace LittleBigMouse.Zoning;

public static class RectExtensions
{
    /// <summary>
    /// 
    /// </summary>
    /// <param name="source"></param>
    /// <param name="target"></param>
    /// <param name="allowed"></param>
    /// <returns></returns>
    public static Rect[] TravelPixels(this Rect source, Rect target, Rect[] allowed)
    {
        var list = source.Reachable(target);

        if (list.Length>1) return list;

        if (!allowed.Any()) return Array.Empty<Rect>();
            
        foreach(var next in allowed)
        {
            var newAllowed = allowed.Where(r => !r.Equals(next)).ToArray();

            var tail = next.TravelPixels(target,newAllowed);
            if (!tail.Any()) continue;

            //todo : should retain smallest travel 
            var travel = source.TravelPixels(next,newAllowed);

            if(travel.Any()) return travel.Concat(tail).ToArray();
        }

        return Array.Empty<Rect>();
    }

    public static Rect[] Reachable(this Rect source, Rect target)
    {
        var left   = Math.Max(target.Left, source.Left);
        var right  = Math.Min(target.Right, source.Right);
        var top    = Math.Max(target.Top, source.Top);
        var bottom = Math.Min(target.Bottom, source.Bottom);

        if(left >= right)
        {
            if(top >= bottom)
            {
                return new[] { source };
            }

            var start = new Rect(source.Left, top, source.Width, bottom - top);
            var dest  = new Rect(target.Left, top, target.Width, bottom - top);
            return  new[] { start, dest };
        }

        if(top >= bottom) 
        {
            var start = new Rect(left, source.Top, right-left, source.Height);
            var dest  = new Rect(left, target.Top, right-left, target.Height);
            return  new[] { start, dest };
        }

        var rect = new Rect(left, top, right-left, bottom - top);
        return  new[] { rect, rect };
    }

}