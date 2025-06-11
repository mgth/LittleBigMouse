using HLab.Geo;

namespace LittleBigMouse.Zoning;

public static class RectExtensions
{
    
    public static Rect[] TravelPathDepth(this Rect start, Rect target, Rect[] allowed)
    {
        var list = start.DetermineContactZones(target);

        if (list.Length>1) return list;

        if (!allowed.Any()) return [];
            
        foreach(var next in allowed)
        {
            var newAllowed = allowed.Where(r => !r.Equals(next)).ToArray();

            var tail = next.TravelPathDepth(target,newAllowed);
            if (tail.Length == 0) continue;

            var head = start.TravelPathDepth(next,newAllowed);
            if(head.Length != 0) return head.Concat(tail).ToArray();
        }

        return [];
    }

    
    
    /// <summary>
    /// Calculates the travel path between a source rectangle and a target rectangle,
    /// potentially navigating through allowed intermediary rectangles.
    /// </summary>
    /// <param name="source">The source rectangle where the travel begins.</param>
    /// <param name="target">The destination rectangle to reach.</param>
    /// <param name="allowed">An array of intermediate rectangles that can be used during the travel.</param>
    /// <returns>
    /// An array of rectangles representing the travel path from the source to the target,
    /// passing through allowed rectangles if necessary.
    /// </returns>
    public static Rect[] TravelPath(this Rect source, Rect target, Rect[] allowed)
    {
        var directPath = source.DetermineContactZones(target);

        // If the source and target rectangles are already connected, return the list directly
       if (directPath.Length > 1) return directPath;
       if (allowed.Length == 0) return [];
        
        var queue = new Queue<(Rect current, Rect[] allowed)>();
        queue.Enqueue((source, [source]));
       
        do
        {
            var (current, currentPath) = queue.Dequeue();
            
            foreach(var next in allowed)
            {
                directPath = current.DetermineContactZones(next);

                // If the source and target rectangles are already connected, return the list directly
                if (directPath.Length > 1) return directPath;
                
                var newAllowed = allowed.Where(r => !r.Equals(next)).ToArray();

                queue.Enqueue((next, newAllowed));
                var tail = next.TravelPath(target,newAllowed);
                if (tail.Length == 0) continue;

                 var travel = source.TravelPath(next,newAllowed);

                if(travel.Length != 0) return travel.Concat(tail).ToArray();
            }
        } while (queue.Count > 0);
            

        return [];
    }

    /// <summary>
    /// Determines the parts of source and target rectangles that allow to horizontally or vertically reach from the source to the target rectangle.
    /// </summary>
    /// <param name="source">The source rectangle from which the path starts.</param>
    /// <param name="target">The target rectangle to determine the path to.</param>
    /// <returns>
    /// An array of rectangles representing the path between the source and target rectangles.
    /// </returns>

    public static Rect[] DetermineContactZones (this Rect source, Rect target)
    {
        var left   = Math.Max(target.Left, source.Left);
        var right  = Math.Min(target.Right, source.Right);
        var top    = Math.Max(target.Top, source.Top);
        var bottom = Math.Min(target.Bottom, source.Bottom);

        if(left >= right)
        {
            if(top >= bottom)
            {
                // No direct path, return the source rectangle
                // +---+          +---+
                // | S |          | 0 |
                // +---+       => +---+
                //      +---+          ....
                //      | T |          .  .
                //      +---+          ....
                
                return [source];
            }
            
            // Horizontal path
            // +---+           .....
            // | S | +---+ ==> +---+ +---+
            // |   | |   |     | 0 | | 1 |
            // +---+ | T |     +---+ +---+
            //       +---+           .....

            var start = new Rect(source.Left, top, source.Width, bottom - top);
            var dest  = new Rect(target.Left, top, target.Width, bottom - top);
            return  [start, dest];
        }

        if(top >= bottom) 
        {
            // Vertical path
            // +---+          ..+-+
            // | S |          . |0|
            // +---+    ==>   ..+-+
            //   +---+          +-+..
            //   | T |          |1| .
            //   +---+          +-+..
            
            var start = new Rect(left, source.Top, right-left, source.Height);
            var dest  = new Rect(left, target.Top, right-left, target.Height);
            return  [start, dest];
        }
        
        
        // Overlaping path
        // +------+            .......
        // | S +------+        .  +--+...
        // |   | T    |        .  |01|  .
        // +---|      |    ==> ...+--+  .
        //     +------+           .......
 
        var rect = new Rect(left, top, right-left, bottom - top);
        return  [rect, rect];
    }

}