using Avalonia.Media;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>
/// The one place that turns a section's settings into a colour, shared by the
/// layout map and the on-screen overlay so a stretch of edge never looks like one
/// thing in the miniature and another on the real monitor.
/// </summary>
public static class BorderSectionBrushes
{
    public static readonly IBrush Blocked = new SolidColorBrush(Color.FromArgb(0xC0, 0xC0, 0x39, 0x2B));
    public static readonly IBrush HalfBlocked = new SolidColorBrush(Color.FromArgb(0xC0, 0xE0, 0x8E, 0x2B));
    public static readonly IBrush Resisting = new SolidColorBrush(Color.FromArgb(0xC0, 0x2B, 0x84, 0xC0));
    public static readonly IBrush Free = new SolidColorBrush(Color.FromArgb(0x80, 0x70, 0x70, 0x70));

    public static IBrush For(double move, bool moveBlock, double drag, bool dragBlock)
    {
        if (moveBlock && dragBlock) return Blocked;
        if (moveBlock || dragBlock) return HalfBlocked;
        if (move > 0 || drag > 0) return Resisting;
        return Free;
    }

    public static IBrush For(IBorderSection section) =>
        For(section.Move, section.MoveBlock, section.Drag, section.DragBlock);

    public static IBrush For(IBorderSide side) =>
        For(side.Move, side.MoveBlock, side.Drag, side.DragBlock);

    public static string Describe(double move, bool moveBlock, double drag, bool dragBlock)
    {
        var m = moveBlock ? "blocked" : $"{move:0.#} mm";
        var d = dragBlock ? "blocked" : $"{drag:0.#} mm";
        return $"Move: {m}\nDrag: {d}";
    }
}
