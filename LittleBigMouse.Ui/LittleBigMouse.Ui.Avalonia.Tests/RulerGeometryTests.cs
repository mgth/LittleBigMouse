using System.Linq;
using LittleBigMouse.Plugin.Layout.Avalonia.Rulers;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The arithmetic behind a ruler, checked without a window. A ruler measures one monitor edge
/// but is drawn across a longer axis, so most of what can go wrong is about the overhang: which
/// millimetres fall outside the edge, where the drawn ruler stops, and what happens when the
/// zero is scrolled off-screen entirely.
/// </summary>
public sealed class RulerGeometryTests
{
    /// <summary>A 300 mm edge drawn on a 400 mm axis, starting 50 mm in — overhang on both sides.</summary>
    static RulerGeometry Centred() => new(
        AxisLength: 400, RulerStart: 0, RulerLength: 300, Zero: 50, Ratio: 1);

    static RulerGraduation At(RulerGeometry geometry, double position) =>
        geometry.Graduations().Single(g => g.Position == position);

    [Fact]
    public void TheThreeBandsTileTheAxisWithoutOverlap()
    {
        var g = Centred();

        Assert.Equal(new RulerBand(0, 50), g.OutsideBefore);
        Assert.Equal(new RulerBand(50, 350), g.Inside);
        Assert.Equal(new RulerBand(350, 400), g.OutsideAfter);
    }

    [Fact]
    public void ARulerStartingAtTheAxisOriginHasNothingBeforeIt()
    {
        var g = Centred() with { Zero = 0 };

        Assert.Null(g.OutsideBefore);
        Assert.Equal(new RulerBand(0, 300), g.Inside);
    }

    [Fact]
    public void ARulerLongerThanTheAxisFillsItAndOverflowsNowhere()
    {
        var g = Centred() with { Zero = 0, RulerLength = 900 };

        Assert.Null(g.OutsideBefore);
        Assert.Null(g.OutsideAfter);
        Assert.Equal(new RulerBand(0, 400), g.Inside);
    }

    [Fact]
    public void ARulerScrolledEntirelyPastTheAxisHasNoInsideBand()
    {
        // Zero beyond the axis: everything visible is overhang.
        var g = Centred() with { Zero = 500 };

        Assert.Null(g.Inside);
        Assert.Equal(new RulerBand(0, 400), g.OutsideBefore);
    }

    [Fact]
    public void ARulerEndingBeforeTheAxisStartsHasNoInsideBandEither()
    {
        var g = Centred() with { Zero = -400, RulerLength = 300 };

        Assert.Null(g.Inside);
        Assert.Equal(new RulerBand(0, 400), g.OutsideAfter);
    }

    [Fact]
    public void GraduationsStartACentimetreBeforeTheWindowAndStopAtTheAxis()
    {
        // The lead-in exists so a decimetre label whose tick has just scrolled off still
        // reaches the edge it belongs to.
        var g = Centred();
        var graduations = g.Graduations().ToList();

        Assert.Equal(40, graduations[0].Position);
        Assert.True(graduations[^1].Position < 400);
        Assert.Equal(399, graduations[^1].Position);
    }

    [Fact]
    public void EveryMillimetreOfTheAxisGetsExactlyOneGraduation()
    {
        var g = Centred();
        var positions = g.Graduations().Select(x => x.Position).ToList();

        Assert.Equal(positions.Count, positions.Distinct().Count());
        Assert.Equal(1, positions[1] - positions[0]);
    }

    [Theory]
    [InlineData(0, RulerGraduationKind.Decimetre, 20.0, "0")]
    [InlineData(100, RulerGraduationKind.Decimetre, 20.0, "1")]
    [InlineData(200, RulerGraduationKind.Decimetre, 20.0, "2")]
    [InlineData(50, RulerGraduationKind.FiveCentimetres, 15.0, "5")]
    [InlineData(150, RulerGraduationKind.FiveCentimetres, 15.0, "5")]
    [InlineData(10, RulerGraduationKind.Centimetre, 10.0, "1")]
    [InlineData(130, RulerGraduationKind.Centimetre, 10.0, "3")]
    [InlineData(5, RulerGraduationKind.FiveMillimetres, 5.0, null)]
    [InlineData(1, RulerGraduationKind.Millimetre, 2.5, null)]
    [InlineData(7, RulerGraduationKind.Millimetre, 2.5, null)]
    public void EachMillimetreGetsTheCoarsestGraduationItQualifiesFor(
        int mm, RulerGraduationKind kind, double tick, string? label)
    {
        // 100 divides by 50, 10 and 5, so the order of the tests is the whole of the rule:
        // a decimetre must not be drawn as a mere centimetre.
        var g = Centred();

        var graduation = At(g, 50 + mm);

        Assert.Equal(kind, graduation.Kind);
        Assert.Equal(tick, graduation.TickLength);
        Assert.Equal(label, graduation.Label);
    }

    [Fact]
    public void CentimetreLabelsRestartAtEveryDecimetre()
    {
        // They number the centimetre within its decimetre, so the ruler reads 1..9 and starts
        // over rather than running to 29.
        var g = Centred();

        Assert.Equal("9", At(g, 50 + 90).Label);
        Assert.Equal("1", At(g, 50 + 110).Label);
        Assert.Equal("9", At(g, 50 + 290).Label);
    }

    [Fact]
    public void GraduationsBeyondEitherEndOfTheMeasuredEdgeAreMarkedOutside()
    {
        // This is what dims the overhang. Getting it wrong makes the ruler look as though the
        // monitor extends past where it does.
        var g = Centred();

        Assert.False(At(g, 50 - 10).Inside);
        Assert.True(At(g, 50).Inside);
        Assert.True(At(g, 50 + 300).Inside);
        Assert.False(At(g, 50 + 301).Inside);
    }

    [Fact]
    public void LabelSizesScaleWithTheDisplayRatio()
    {
        // The ruler is drawn in display units, so a label sized in millimetres would shrink to
        // nothing as the layout zooms out.
        var g = Centred() with { Ratio = 3 };

        Assert.Equal(15.0, At(g, 50 + 100).LabelSize); // decimetre: 5 * 3
        Assert.Equal(12.0, At(g, 50 + 50).LabelSize);  // five centimetres: 4 * 3
        Assert.Equal(9.0, At(g, 50 + 10).LabelSize);   // centimetre: 3 * 3
        Assert.Equal(0.0, At(g, 50 + 1).LabelSize);    // unlabelled
    }

    [Fact]
    public void AWindowScrolledIntoTheRulerStartsItsGraduationsThere()
    {
        // RulerStart is the visible window into a ruler longer than the axis; the lead-in is
        // relative to it, not to the ruler's zero.
        var g = Centred() with { RulerStart = 120, Zero = 0 };

        Assert.Equal(110, g.Graduations().First().Position);
    }

    [Fact]
    public void NegativeGraduationsKeepSignedLabels()
    {
        // The overhang before zero is numbered too, and integer division has to carry the sign
        // rather than fold -100 onto 1.
        var g = Centred() with { RulerStart = -250, Zero = 300 };

        Assert.Equal("-1", At(g, 300 - 100).Label);
        Assert.Equal("-2", At(g, 300 - 200).Label);
        Assert.Equal("-3", At(g, 300 - 30).Label);
    }

    [Fact]
    public void TheHalfDecimetreIsSignedLikeEveryOtherNumber()
    {
        // It used to be a hardcoded "5", which read as +5 cm in the middle of an overhang whose
        // every other label was negative.
        // A wider axis than Centred()'s, so the run reaches past the zero on both sides.
        var g = Centred() with { AxisLength = 600, RulerStart = -250, Zero = 300 };

        Assert.Equal("-5", At(g, 300 - 50).Label);
        Assert.Equal("-5", At(g, 300 - 150).Label);
        Assert.Equal("5", At(g, 300 + 50).Label);
        Assert.Equal("5", At(g, 300 + 150).Label);
    }
}
