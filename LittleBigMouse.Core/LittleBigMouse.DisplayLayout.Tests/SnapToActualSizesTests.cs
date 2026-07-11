using LittleBigMouse.Platform.Linux;
using P = LittleBigMouse.Platform.Linux.LinuxDisplayController.PlacedOutput;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The apply-layout positions are computed against PREDICTED logical sizes
/// (round(native/scale)); the compositor's own rounding is authoritative and can
/// differ by a pixel. SnapToActualSizes rebuilds the intended edge contacts with
/// the actual sizes so a contact never silently becomes a gap or an overlap.
/// </summary>
public class SnapToActualSizesTests
{
    static void AssertAt(Dictionary<string, (double X, double Y)> result, string name, double x, double y)
    {
        Assert.Equal(x, result[name].X, 2);
        Assert.Equal(y, result[name].Y, 2);
    }

    [Fact]
    public void ActualSizeSmaller_ClosesTheGap()
    {
        var result = LinuxDisplayController.SnapToActualSizes(
        [
            new P("A", 0, 0, 1000, 1000, 1000, 1000),
            new P("B", 1000, 0, 2000, 1000, 1999, 1000),
            new P("C", 3000, 0, 1000, 1000, 1000, 1000),
        ]);

        AssertAt(result, "B", 1000, 0);
        AssertAt(result, "C", 2999, 0);
    }

    [Fact]
    public void ActualSizeBigger_PushesTheNeighbourAway()
    {
        var result = LinuxDisplayController.SnapToActualSizes(
        [
            new P("A", 0, 0, 1000, 1000, 1001, 1000),
            new P("B", 1000, 0, 1000, 1000, 1000, 1000),
        ]);

        AssertAt(result, "B", 1001, 0);
    }

    [Fact]
    public void VerticalContact_ChainsOnActualHeights()
    {
        var result = LinuxDisplayController.SnapToActualSizes(
        [
            new P("A", 0, 0, 1000, 500, 1000, 499),
            new P("B", 0, 500, 1000, 500, 1000, 500),
        ]);

        AssertAt(result, "B", 0, 499);
    }

    [Fact]
    public void NoContact_KeepsIntendedPositions()
    {
        var result = LinuxDisplayController.SnapToActualSizes(
        [
            new P("A", 0, 0, 1000, 1000, 999, 1000),
            new P("B", 5000, 0, 1000, 1000, 1000, 1000),
        ]);

        AssertAt(result, "A", 0, 0);
        AssertAt(result, "B", 5000, 0);
    }

    [Fact]
    public void FourOutputRow_MixedDrift_KeepsEveryContact()
    {
        // the maintainer's row: DELL exact, PHL 1px smaller, SAM 1px bigger, TV exact
        var result = LinuxDisplayController.SnapToActualSizes(
        [
            new P("DP-1", 0, 624, 1280, 720, 1280, 720),
            new P("DP-2", 1280, 308, 3072, 1728, 3071, 1728),
            new P("DP-3", 4352, 0, 3072, 1728, 3073, 1728),
            new P("HDMI-A-1", 7424, 0, 2649, 1490, 2649, 1490),
        ]);

        AssertAt(result, "DP-1", 0, 624);
        AssertAt(result, "DP-2", 1280, 308);
        AssertAt(result, "DP-3", 4351, 0);
        AssertAt(result, "HDMI-A-1", 7424, 0);
    }

    [Fact]
    public void PredictionExact_IsANoOp()
    {
        var result = LinuxDisplayController.SnapToActualSizes(
        [
            new P("A", 0, 0, 1000, 1000, 1000, 1000),
            new P("B", 1000, 0, 1000, 1000, 1000, 1000),
        ]);

        AssertAt(result, "A", 0, 0);
        AssertAt(result, "B", 1000, 0);
    }
}
