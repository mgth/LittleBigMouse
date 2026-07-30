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
    /// <summary>Nothing crosses here, in either mode.</summary>
    public static readonly IBrush Blocked = new SolidColorBrush(Color.FromArgb(0xC0, 0xC0, 0x39, 0x2B));

    /// <summary>A wall for the cursor alone: a window can still be dragged across.</summary>
    public static readonly IBrush MoveBlocked = new SolidColorBrush(Color.FromArgb(0xC0, 0xE0, 0x8E, 0x2B));

    /// <summary>The opposite: the cursor passes, a drag does not.</summary>
    public static readonly IBrush DragBlocked = new SolidColorBrush(Color.FromArgb(0xC0, 0x9B, 0x59, 0xB6));

    /// <summary>Resisting, but crossable in both modes.</summary>
    public static readonly IBrush Resisting = new SolidColorBrush(Color.FromArgb(0xC0, 0x2B, 0x84, 0xC0));

    public static readonly IBrush Free = new SolidColorBrush(Color.FromArgb(0x80, 0x70, 0x70, 0x70));

    /// <summary>
    /// One colour per state, the two half-blocks included: they are opposite
    /// settings — a barrier the cursor cannot pass but a window can, versus the
    /// reverse — and sharing an amber made them indistinguishable on the bands,
    /// where the colour is all there is to go on.
    /// </summary>
    public static IBrush For(double move, bool moveBlock, double drag, bool dragBlock)
    {
        if (moveBlock && dragBlock) return Blocked;
        if (moveBlock) return MoveBlocked;
        if (dragBlock) return DragBlocked;
        if (move > 0 || drag > 0) return Resisting;
        return Free;
    }

    public static IBrush For(IBorderSection section) =>
        For(section.Move, section.MoveBlock, section.Drag, section.DragBlock);

    public static string Describe(double move, bool moveBlock, double drag, bool dragBlock)
    {
        var m = moveBlock ? "blocked" : $"{move:0.#} mm";
        var d = dragBlock ? "blocked" : $"{drag:0.#} mm";
        return $"Move: {m}\nDrag: {d}";
    }
}
