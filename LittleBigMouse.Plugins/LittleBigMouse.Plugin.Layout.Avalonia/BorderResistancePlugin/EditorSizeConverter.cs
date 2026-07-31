using System;
using System.Globalization;
using Avalonia;
using Avalonia.Data.Converters;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>
/// The room a panel drawn over a monitor may take on one axis: everything inside
/// the bands running along that monitor's edges, up to a ceiling.
/// <para>
/// <see cref="HLab.Mvvm.Avalonia.Converters.ScaleConverter"/> takes a share of the
/// monitor's SMALLER side and clamps it between a floor and a ceiling. Two things
/// went wrong with that here. The floor outgrew a small monitor, so the panel
/// covered the bands — and the bands are the only way to reach a section. And one
/// figure for both axes squares off a panel that is wider than it is tall: on a
/// 16:9 frame it was penned into the height while the whole width sat unused.
/// </para>
/// <para>
/// So: each axis is measured on its own, the inset for the bands comes off both
/// ends of it, and what is left is offered whole. The share is gone with the
/// floor — it existed to keep the panel from swallowing a monitor, which is the
/// ceiling's job, and on a small monitor there is nothing to spare anyway.
/// </para>
/// </summary>
public class EditorSizeConverter : IValueConverter
{
    /// <param name="parameter">
    /// axis|ceiling|inset — axis is <c>width</c>, <c>height</c> or the smaller of
    /// the two; a ceiling of zero means none. Only one axis needs a ceiling: with
    /// uniform scaling the tighter of the two settles the size, and capping the
    /// other as well would crop a panel whose natural shape is wider than tall.
    /// </param>
    public object? Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (parameter is not string s) return null;
        if (targetType != typeof(double)) return null;

        var bounds = value switch
        {
            Rect r => r,
            _ => default
        };

        var p = s.Split('|');

        var axis = p[0].Trim().ToLowerInvariant() switch
        {
            "width" => bounds.Width,
            "height" => bounds.Height,
            _ => Math.Min(bounds.Width, bounds.Height)
        };

        if (double.IsNaN(axis) || double.IsInfinity(axis)) axis = 0;

        var ceiling = Parse(p, 1, 0);
        var inset = Parse(p, 2, 0);

        if (ceiling <= 0) ceiling = double.MaxValue;

        // A monitor drawn no bigger than its own bands leaves nothing, and asks
        // for nothing: a negative size is not a size.
        return Math.Max(Math.Min(axis - 2 * inset, ceiling), 0);
    }

    static double Parse(string[] parts, int index, double fallback) =>
        parts.Length > index && double.TryParse(parts[index], NumberStyles.Float, CultureInfo.InvariantCulture, out var v)
            ? v
            : fallback;

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) =>
        throw new NotSupportedException("EditorSizeConverter : ConvertBack");
}
