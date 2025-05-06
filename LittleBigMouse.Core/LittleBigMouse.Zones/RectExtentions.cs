using HLab.Geo;

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

        if (!allowed.Any()) return [];
            
        foreach(var next in allowed)
        {
            var newAllowed = allowed.Where(r => !r.Equals(next)).ToArray();

            var tail = next.TravelPixels(target,newAllowed);
            if (!tail.Any()) continue;

            //todo : should retain the smallest travel 
            var travel = source.TravelPixels(next,newAllowed);

            if(travel.Any()) return travel.Concat(tail).ToArray();
        }

        return [];
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
                return [source];
            }

            var start = new Rect(source.Left, top, source.Width, bottom - top);
            var dest  = new Rect(target.Left, top, target.Width, bottom - top);
            return  [start, dest];
        }

        if(top >= bottom) 
        {
            var start = new Rect(left, source.Top, right-left, source.Height);
            var dest  = new Rect(left, target.Top, right-left, target.Height);
            return  [start, dest];
        }

        var rect = new Rect(left, top, right-left, bottom - top);
        return  [rect, rect];
    }

}