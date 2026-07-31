using Avalonia.Media;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>
/// One colour per kind of snap reference, so a guide says what a boundary caught
/// and not merely that it caught something.
/// <para>
/// Kept clear of <see cref="BorderSectionBrushes"/>: those describe what a section
/// DOES, these describe what it lined up WITH, and confusing the two palettes would
/// make a guide read as a state.
/// </para>
/// </summary>
public static class SnapGuideBrushes
{
    /// <summary>This edge's own ends and middle — the monitor's own geometry.</summary>
    public static readonly IBrush Own = Brushes.White;

    /// <summary>Another screen's visible edge.</summary>
    public static readonly IBrush ScreenEdge = new SolidColorBrush(Color.FromRgb(0x33, 0xC4, 0xD6));

    /// <summary>A section already drawn, on this edge or on one running the same way.</summary>
    public static readonly IBrush Section = new SolidColorBrush(Color.FromRgb(0xE8, 0xC3, 0x3A));

    public static IBrush For(SnapKind kind) => kind switch
    {
        SnapKind.ScreenEdge => ScreenEdge,
        SnapKind.Section => Section,
        _ => Own
    };
}
