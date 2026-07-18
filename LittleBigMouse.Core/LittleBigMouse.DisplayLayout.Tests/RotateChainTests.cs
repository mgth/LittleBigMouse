using LittleBigMouse.DisplayLayout.Dimensions;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// A freshly built Rotate(1) chain must expose the transposed size synchronously: the
/// factory places monitors right after building it, so a deferred first value would let
/// the placement run on the untransposed size. The source must be the INTRINSIC (EDID,
/// landscape) size — the chain owns the transposition (#507).
/// </summary>
public class RotateChainTests
{
    static DisplaySizeInMm Landscape()
    {
        var size = new DisplaySizeInMm();
        size.Width = 697;
        size.Height = 392;
        return size;
    }

    [Fact]
    public void Rotate1_ExposesTransposedSize_Immediately()
    {
        var rotated = Landscape().Rotate(1);

        Assert.Equal(392, rotated.Width);
        Assert.Equal(697, rotated.Height);
    }

    [Fact]
    public void FullDepthChain_Rotate1_ExposesTransposedSize_Immediately()
    {
        var chain = Landscape().Rotate(1).Scale(new DisplayRatioValue(1.0, 1.0)).Locate();

        Assert.Equal(392, chain.Width);
        Assert.Equal(697, chain.Height);
    }
}
