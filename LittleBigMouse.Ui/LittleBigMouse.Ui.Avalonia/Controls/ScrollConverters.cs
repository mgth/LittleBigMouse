using Avalonia;
using Avalonia.Data.Converters;

namespace LittleBigMouse.Ui.Avalonia.Controls;

public static class ScrollConverters
{
    /// <summary>
    /// True once a scroll viewer has moved off its top. For headers that stay put but
    /// give up their explanation as soon as they stop being the thing you are looking
    /// at.
    /// </summary>
    /// <remarks>
    /// The few pixels of slack keep a pinned header from flickering between its two
    /// shapes when the content sits a hair off zero — a scroll of one pixel is not a
    /// scroll.
    /// </remarks>
    public static readonly FuncValueConverter<Vector, bool> IsScrolled = new(offset => offset.Y > 4);
}
