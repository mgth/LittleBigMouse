using Avalonia;
using DynamicData;
using HLab.Sys.Windows.API;
using System;

namespace HLab.Sys.Windows.Monitors;

internal static class WinApiExtensions
{
    public static Rect ToRect(this WinDef.Rect r)
    {
        return new Rect(r.X, r.Y, r.Width, r.Height);
    }
    public static Point ToPoint(this WinDef.Point p)
    {
        return new Point(p.X, p.Y);
    }

    public static T GetOrAdd<T,TKey>(this SourceCache<T,TKey> @this, ISystemMonitorsService service, TKey key, Func<ISystemMonitorsService,TKey,T> get)
    {
        var lookup = @this.Lookup(key);
        if(lookup.HasValue) return lookup.Value;
        var value = get(service,key);
        @this.AddOrUpdate(value);
        return value;
    }
}