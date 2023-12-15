using Avalonia;
using System;
using System.Collections.Generic;
using System.Linq;

namespace LittleBigMouse.DisplayLayout.Monitors;

public static class MonitorExtensions
{
    public static readonly Thickness Infinity = new(double.PositiveInfinity);

    public static bool IsPositiveInfinity(this Thickness @this)
    {
        return @this is { Left: double.PositiveInfinity, Top: double.PositiveInfinity, Right: double.PositiveInfinity, Bottom: double.PositiveInfinity };
    }

    /// <summary>
    /// Distance for this monitor to touch another, or infinity if they do not touch.
    /// </summary>
    /// <param name="this"></param>
    /// <param name="other"></param>
    /// <param name="zero"></param>
    /// <returns></returns>
    public static Thickness DistanceToTouch(this Rect @this, Rect other, bool zero = false)
    {
        var distance = @this.Distance(other);
        if(distance.Top > 0 || distance.Bottom > 0 || zero && (distance.Top == 0 || distance.Bottom == 0))
        {
            if(distance.Left > 0 || distance.Right > 0 || zero && (distance.Left == 0 || distance.Right == 0))
            {
                return Infinity;
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

    /// <summary>
    /// Distance from this monitor borders to another monitor opposite border.
    /// </summary>
    /// <param name="this"></param>
    /// <param name="other"></param>
    /// <returns></returns>
    public static Thickness Distance(this Rect @this, Rect other)
    {
        return new Thickness(
            @this.X - other.Right,
            @this.Y - other.Bottom,
            other.X - @this.Right,
            other.Y - @this.Bottom
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

    public static Thickness Distance(this Rect @this, IEnumerable<Rect> others)
    {
        var min = new Thickness(double.MaxValue);

        foreach (var other in others)
        {
            min = min.Min(@this.Distance(other));
        }
        return min;
    }


    public static Thickness DistanceToTouch(
        this Rect @this, 
        IEnumerable<Rect> others,
        bool zero = false)
    {
        var min = new Thickness(double.PositiveInfinity);

        foreach (var other in others)
        {
            min = min.Min(@this.DistanceToTouch(other, zero));
        }
        return min;
    }

    public static double DistanceHV(this Thickness distance)
    {
        var x = distance.Left >= 0 ? distance.Left : distance.Right >= 0 ? distance.Right : Math.Max(distance.Left, distance.Right);
        var y = distance.Top >= 0 ? distance.Top : distance.Bottom >= 0 ? distance.Bottom : Math.Max(distance.Top, distance.Bottom);

        var v = new Vector(x,y);

        if (v is { X: >= 0, Y: >= 0 }) return v.Length;

        if (v.X >= 0) return v.X;
        if (v.Y >= 0) return v.Y;

        return Math.Max(v.X, v.Y);
    }

    public static double MinPositive(this Thickness @this)
    {
        var src = @this.ToArray().Where(d => d >= 0.0).ToArray();
        return src.Length == 0 ? double.PositiveInfinity : src.Min();
    }

    public static double[] ToArray(this Thickness distance) => [distance.Left, distance.Top, distance.Right, distance.Bottom];
}