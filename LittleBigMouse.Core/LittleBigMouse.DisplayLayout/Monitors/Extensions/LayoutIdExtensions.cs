#nullable enable
using System.Linq;

namespace LittleBigMouse.DisplayLayout.Monitors.Extensions;

public static class LayoutIdExtensions
{
    /// <summary>
    /// Persistence key for the current monitor set. A rotated display contributes its
    /// orientation to the key (v4 behavior lost in the v5 port): reloading positions
    /// saved for the other orientation produces a disjoint layout, so each orientation
    /// must map to its own stored config. Landscape (orientation 0) adds no suffix so
    /// every pre-existing stored key stays valid.
    /// </summary>
    public static string ComputeId(this IMonitorsLayout layout)
        => string.Join("+", layout.PhysicalMonitors
            .Select(m =>
            {
                var orientation = m.ActiveSource?.Source.Orientation ?? 0;
                return orientation == 0 ? m.Id : $"{m.Id}_{orientation}";
            })
            .OrderBy(s => s));
}
