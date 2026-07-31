using DynamicData;
using HLab.Base.ReactiveUI;
using LittleBigMouse.DisplayLayout.Dimensions;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The counter the UI's live preview is built on. Only its monotonicity is asserted —
/// it is process-wide and test classes run in parallel, so absolute values mean nothing
/// and a concurrent bump can only make a "did it move" assertion more true.
/// </summary>
public class UnsavedRevisionTests
{
    [Fact]
    public void EveryEditMovesIt_IncludingTheOnesSavedCannotSignal()
    {
        // This is the whole point. Saved goes true→false once and then sits there: the
        // second edit of an already-unsaved model raises nothing, which is why
        // persistence has to walk the tree marking everything saved again before the
        // reactive chains work at all. A consumer that wants to know whether anything
        // moved needs this instead.
        var section = new BorderSection { From = 0, To = 10 };
        section.Saved = true;

        var start = SavableReactiveModel.Revision;

        section.To = 20;
        var afterFirst = SavableReactiveModel.Revision;
        Assert.True(afterFirst > start);
        Assert.False(section.Saved);

        section.To = 30;
        Assert.True(
            SavableReactiveModel.Revision > afterFirst,
            "the second edit is exactly what the Saved transition cannot carry");
    }

    [Fact]
    public void ItMovesForAChildDeepInTheTree()
    {
        // Nothing is wired between a section and the layout for this to work: the count
        // sits on the flag, so nesting is irrelevant.
        var side = new BorderSide();
        var section = new BorderSection { From = 0, To = 10 };
        side.Sections.Add(section);
        side.Saved = true;
        section.Saved = true;

        var start = SavableReactiveModel.Revision;
        section.Move = 5;

        Assert.True(SavableReactiveModel.Revision > start);
    }
}
