using System;
using System.Collections.Generic;

namespace LittleBigMouse.Plugin.Layout.Avalonia.Rulers;

/// <summary>
/// Which subdivision a graduation marks. Decides how far its tick reaches and what number,
/// if any, it carries.
/// </summary>
public enum RulerGraduationKind
{
    Millimetre,
    FiveMillimetres,
    Centimetre,
    FiveCentimetres,
    Decimetre,
}

/// <summary>One mark on the ruler, in ruler coordinates — millimetres along the axis.</summary>
/// <param name="Position">Distance from the start of the drawn axis, not from the ruler's zero.</param>
/// <param name="TickLength">How far the tick reaches away from the axis.</param>
/// <param name="Label">The number this graduation carries, or null when it carries none.</param>
/// <param name="LabelSize">Font size for <paramref name="Label"/>, already scaled to the display.</param>
/// <param name="Inside">
/// Whether this millimetre falls within the monitor the ruler measures. Graduations run past
/// both ends, and the ones outside are drawn dimmer rather than skipped — that overhang is how
/// you see where the neighbouring monitor starts.
/// </param>
public readonly record struct RulerGraduation(
    double Position,
    RulerGraduationKind Kind,
    double TickLength,
    string? Label,
    double LabelSize,
    bool Inside);

/// <summary>An interval along the ruler axis, in ruler coordinates.</summary>
public readonly record struct RulerBand(double Start, double End);

/// <summary>
/// What a ruler is made of, worked out with no <c>DrawingContext</c> and no orientation: the
/// three background bands and the sequence of graduations, all in ruler coordinates
/// (millimetres along the axis, growing away from the axis start).
/// <para>
/// Turning those into screen coordinates is <see cref="RulerOrientation"/>'s job, and putting
/// ink on them is the control's. The split is what makes the arithmetic below checkable: which
/// millimetre gets a number, where the drawn ruler stops and the overhang begins, what happens
/// when the ruler starts off-screen — none of that needs a window to answer.
/// </para>
/// </summary>
/// <param name="AxisLength">Length of the drawn axis, in millimetres.</param>
/// <param name="RulerStart">Where the visible window into the ruler begins, in millimetres.</param>
/// <param name="RulerLength">Length of the monitor edge being measured, in millimetres.</param>
/// <param name="Zero">Where the measured edge's zero falls along the drawn axis.</param>
/// <param name="Ratio">Display units per millimetre, used to size the labels.</param>
public readonly record struct RulerGeometry(
    double AxisLength,
    double RulerStart,
    double RulerLength,
    double Zero,
    double Ratio)
{
    // How far each kind of tick reaches. A decimetre reads at a glance because it is twice a
    // centimetre, and a millimetre is barely a notch.
    const double DecimetreTick = 20.0;
    const double FiveCentimetreTick = 15.0;
    const double CentimetreTick = 10.0;
    const double FiveMillimetreTick = 5.0;
    const double MillimetreTick = 2.5;

    // Label sizes before scaling to the display.
    const double DecimetreLabel = 5.0;
    const double FiveCentimetreLabel = 4.0;
    const double CentimetreLabel = 3.0;

    /// <summary>Where the measured edge ends along the drawn axis.</summary>
    public double End => Zero + RulerLength;

    /// <summary>
    /// The overhang before the measured edge starts, or null when the axis begins at or after
    /// the zero — a ruler scrolled so its zero is off-screen has no "before" to draw.
    /// </summary>
    public RulerBand? OutsideBefore =>
        AxisLength > 0 && Zero > 0 ? new RulerBand(0, Math.Min(Zero, AxisLength)) : null;

    /// <summary>The overhang after the measured edge ends, or null when it runs off the axis.</summary>
    public RulerBand? OutsideAfter =>
        End < AxisLength ? new RulerBand(Math.Max(End, 0), AxisLength) : null;

    /// <summary>
    /// The measured edge itself, clipped to the axis — null when it falls entirely outside.
    /// </summary>
    public RulerBand? Inside =>
        Zero < AxisLength && End > 0
            ? new RulerBand(Math.Max(Zero, 0), Math.Min(End, AxisLength))
            : null;

    /// <summary>
    /// Every graduation the axis shows, in order. Starts a centimetre before the visible window
    /// so a label whose tick has just scrolled off still reaches the edge it belongs to.
    /// </summary>
    public IEnumerable<RulerGraduation> Graduations()
    {
        var mm = (int)RulerStart - 10;
        var position = Zero + mm;

        while (position < AxisLength)
        {
            yield return Describe(mm, position);

            mm++;
            position += 1.0;
        }
    }

    RulerGraduation Describe(int mm, double position)
    {
        // Outside the measured edge on either side; the tick is drawn dimmer, not dropped.
        var inside = mm >= 0 && mm <= RulerLength;

        // Coarsest first. Every step divides the one below it, so the first match wins.
        if (mm % 100 == 0)
            return new(position, RulerGraduationKind.Decimetre, DecimetreTick,
                (mm / 100).ToString(), DecimetreLabel * Ratio, inside);

        // Both of these number the centimetre within its decimetre, so the labels read 1..9 and
        // restart — and carry the sign in the overhang before zero. They differ only in how far
        // the tick reaches and how large the number is drawn: the half-decimetre is the landmark
        // between two numbered decimetres, so it gets more of both.
        if (mm % 50 == 0)
            return new(position, RulerGraduationKind.FiveCentimetres, FiveCentimetreTick,
                (mm % 100 / 10).ToString(), FiveCentimetreLabel * Ratio, inside);

        if (mm % 10 == 0)
            return new(position, RulerGraduationKind.Centimetre, CentimetreTick,
                (mm % 100 / 10).ToString(), CentimetreLabel * Ratio, inside);

        if (mm % 5 == 0)
            return new(position, RulerGraduationKind.FiveMillimetres, FiveMillimetreTick,
                null, 0, inside);

        return new(position, RulerGraduationKind.Millimetre, MillimetreTick, null, 0, inside);
    }
}
