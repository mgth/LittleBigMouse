using System;
using LittleBigMouse.DisplayLayout.Dimensions;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>A boundary being dragged has landed on a snap target, and on which one.</summary>
/// <param name="Side">The edge it belongs to, as the MODEL — the one thing every view of that edge shares.</param>
/// <param name="Mm">Where along the edge, in millimetres from its starting corner.</param>
/// <param name="Kind">What it caught, which is what the guide is coloured by.</param>
/// <param name="LayoutMm">
/// The same place in layout coordinates, so a view that is not this edge's band can
/// still find it: the reference line crosses screens that know nothing of the edge
/// it started from.
/// </param>
/// <param name="IsVertical">
/// Whether the edge runs along Y, which is what makes <paramref name="LayoutMm"/> a
/// height rather than an abscissa — and the line across it horizontal.
/// </param>
public readonly record struct BorderGuide(
    BorderSide Side, double Mm, SnapKind Kind, double LayoutMm, bool IsVertical);

/// <summary>
/// Where the boundary being dragged has caught something, for every view of that
/// edge at once — the same arrangement as <see cref="BorderSectionSelection"/>,
/// and for the same reason.
/// <para>
/// A guide used to be drawn by the band that happened to be handling the gesture,
/// into its own overlay and into the presenter's panel. Drag on a real screen and
/// there is no presenter, so the long line had nowhere to go and the app showed
/// nothing at all — while the edit itself was landing on targets taken from the
/// whole layout. You could snap onto a section three screens away without ever
/// being told.
/// </para>
/// <para>
/// Published here, the guide reaches every band showing that edge: the one on the
/// layout map draws it across itself and carries the line through the neighbouring
/// screens, and the band on the real monitor marks it where the hand is.
/// </para>
/// </summary>
public static class BorderSectionGuide
{
    public static BorderGuide? Current { get; private set; }

    public static event Action<BorderGuide?>? Changed;

    public static void Show(BorderSide side, double mm, SnapKind kind, double layoutMm, bool isVertical)
    {
        var guide = new BorderGuide(side, mm, kind, layoutMm, isVertical);
        if (Current is { } current && current == guide) return;

        Current = guide;
        Changed?.Invoke(guide);
    }

    public static void Clear()
    {
        if (Current == null) return;

        Current = null;
        Changed?.Invoke(null);
    }
}
