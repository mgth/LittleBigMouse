using DynamicData;
using LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;
using Xunit;

using static LittleBigMouse.Ui.Avalonia.Tests.BorderTestLayouts;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// Whole pointer gestures on a band, played out event by event without a display:
/// press, the positions that follow, and whichever of release or cancellation ends
/// them.
/// <para>
/// The band itself only turns Avalonia events into millimetres and the feedback
/// into shapes. Everything a gesture decides — what a press takes hold of, what
/// each position edits, which boundary the guide marks, what a cancellation puts
/// back — is <see cref="BorderSectionGesture"/>, and is all exercised here.
/// </para>
/// </summary>
public sealed class BorderGestureSequenceTests
{
    //==================//
    // Drawing          //
    //==================//

    [Fact]
    public void DrawingCreatesOnlyOnRelease()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var gesture = new BorderSectionGesture(side);

        Assert.Null(gesture.Press(50, snap: false));
        Assert.Equal(BorderSectionGesture.Kind.Draw, gesture.Mode);

        // The sweep only outlines: nothing exists until the button comes up.
        var feedback = gesture.Move(150, snap: false);
        Assert.Equal(new BorderSpan(50, 150), feedback.Preview);
        Assert.Empty(side.Side.Sections.Items);

        var created = gesture.Release(150, snap: false);

        Assert.NotNull(created);
        Assert.Equal(50, created!.From);
        Assert.Equal(150, created.To);
        Assert.False(gesture.Active);
    }

    [Fact]
    public void AClickThatDrawsNothingCreatesNothing()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var gesture = new BorderSectionGesture(side);

        gesture.Press(50, snap: false);

        Assert.Null(gesture.Release(50, snap: false));
        Assert.Empty(side.Side.Sections.Items);
    }

    //==================//
    // Resizing         //
    //==================//

    [Fact]
    public void ResizingHoldsTheBoundaryWhereItWasGrabbed()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(50, 150)!;
        var gesture = new BorderSectionGesture(side);

        // Pressed 3 mm inside the handle rather than on the line, which is the usual
        // case: the handle is 4 mm deep here. The boundary must not move yet.
        gesture.Press(53, snap: false);
        Assert.Equal(50, section.From);

        // A pointer position arrives with the press itself. Before the grab offset
        // was kept, this alone slid the boundary from 50 to 53.
        gesture.Move(53, snap: false);
        Assert.Equal(50, section.From);

        // From there the boundary follows the movement, not the position.
        gesture.Move(63, snap: false);
        Assert.Equal(60, section.From);
        Assert.Equal(150, section.To);
    }

    [Fact]
    public void ResizingTheFarEndHoldsItsGripToo()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(50, 150)!;
        var gesture = new BorderSectionGesture(side);

        gesture.Press(147, snap: false);
        gesture.Move(147, snap: false);
        Assert.Equal(150, section.To);

        gesture.Move(137, snap: false);
        Assert.Equal(140, section.To);
        Assert.Equal(50, section.From);
    }

    [Fact]
    public void EachEndIsResizedInTurnAndStopsWhereItRunsOut()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(50, 150)!;
        var gesture = new BorderSectionGesture(side);

        // The near end, out to the edge's own start and no further: the position is
        // clamped to the edge before anything else looks at it.
        gesture.Press(51, snap: false);
        gesture.Move(1, snap: false);
        Assert.Equal(0, section.From);

        gesture.Move(-40, snap: false);
        Assert.Equal(0, section.From);
        gesture.Release(-40, snap: false);

        // The far end, up against a neighbour rather than against the edge.
        var neighbour = side.Create(200, 260)!;

        gesture.Press(149, snap: false);
        gesture.Move(199, snap: false);
        Assert.Equal(200, section.To);

        // Pushed into the neighbour, it comes to rest on it and does not swallow it.
        gesture.Move(260, snap: false);
        Assert.Equal(200, section.To);
        Assert.Equal(200, neighbour.From);
        Assert.Equal(260, neighbour.To);
    }

    [Fact]
    public void AGestureOnATouchingBoundaryTakesTheSectionItStartsFrom()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        side.Create(50, 150);
        var lower = side.Create(150, 250)!;
        var gesture = new BorderSectionGesture(side);

        // The whole point of the handle rule, end to end: pressing the shared
        // boundary resizes the lower section rather than the one above it.
        Assert.Same(lower, gesture.Press(150, snap: false));

        gesture.Move(150, snap: false);
        gesture.Move(170, snap: false);

        Assert.Equal(170, lower.From);
        Assert.Equal(250, lower.To);
    }

    //==================//
    // Moving           //
    //==================//

    [Fact]
    public void MovingCarriesTheWholeSection()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(50, 150)!;
        var gesture = new BorderSectionGesture(side);

        Assert.Same(section, gesture.Press(100, snap: false));
        Assert.Equal(BorderSectionGesture.Kind.Move, gesture.Mode);

        gesture.Move(120, snap: false);

        Assert.Equal(70, section.From);
        Assert.Equal(170, section.To);
    }

    //==================//
    // Release, undrawn //
    //==================//

    [Fact]
    public void ReleasingWithoutMovingLeavesASectionExactlyWhereItWas()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(50, 150)!;
        var gesture = new BorderSectionGesture(side);

        // A plain click in the middle of a section only selects it.
        Assert.Same(section, gesture.Press(100, snap: true));
        Assert.Null(gesture.Release(100, snap: true));

        Assert.Equal(50, section.From);
        Assert.Equal(150, section.To);
        Assert.False(gesture.Active);
        Assert.Single(side.Side.Sections.Items);
    }

    [Fact]
    public void ReleasingWithoutMovingLeavesAGrabbedBoundaryAlone()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(50, 150)!;
        var gesture = new BorderSectionGesture(side);

        // Clicked on the handle, so the gesture is a resize — but no position ever
        // followed, and a resize that never ran must not snap the boundary on its
        // way out.
        gesture.Press(53, snap: true);
        Assert.Equal(BorderSectionGesture.Kind.ResizeFrom, gesture.Mode);

        Assert.Null(gesture.Release(53, snap: true));
        Assert.Equal(50, section.From);
        Assert.Equal(150, section.To);
    }

    //==================//
    // Cancelling       //
    //==================//

    [Fact]
    public void CancellingAResizePutsBothEndsBack()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(50, 150)!;
        var gesture = new BorderSectionGesture(side);

        gesture.Press(53, snap: false);
        gesture.Move(63, snap: false);
        Assert.Equal(60, section.From);

        Assert.Same(section, gesture.Cancel());

        Assert.Equal(50, section.From);
        Assert.Equal(150, section.To);
        Assert.False(gesture.Active);
    }

    [Fact]
    public void CancellingAResizeOfTheFarEndPutsItBackToo()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(50, 150)!;
        var gesture = new BorderSectionGesture(side);

        gesture.Press(147, snap: false);
        gesture.Move(220, snap: false);
        Assert.Equal(223, section.To);

        Assert.Same(section, gesture.Cancel());

        Assert.Equal(50, section.From);
        Assert.Equal(150, section.To);
    }

    [Fact]
    public void CancellingAMovePutsTheSectionBack()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(50, 150)!;
        var gesture = new BorderSectionGesture(side);

        gesture.Press(100, snap: false);
        gesture.Move(120, snap: false);
        Assert.Equal(70, section.From);

        Assert.Same(section, gesture.Cancel());

        Assert.Equal(50, section.From);
        Assert.Equal(150, section.To);
    }

    [Fact]
    public void CancellingADrawingLeavesNothingBehind()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var gesture = new BorderSectionGesture(side);

        gesture.Press(50, snap: false);
        gesture.Move(150, snap: false);

        // Nothing was ever created, so there is nothing to put back.
        Assert.Null(gesture.Cancel());
        Assert.Empty(side.Side.Sections.Items);

        // The button still has to come up, and must not create the section then.
        Assert.Null(gesture.Release(150, snap: false));
        Assert.Empty(side.Side.Sections.Items);
    }

    [Fact]
    public void PositionsArrivingAfterACancellationAreIgnored()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(50, 150)!;
        var gesture = new BorderSectionGesture(side);

        gesture.Press(100, snap: false);
        gesture.Move(120, snap: false);
        gesture.Cancel();

        // Escape does not take the capture away, so the pointer keeps reporting until
        // the button comes up. The gesture is over and none of it may land.
        var feedback = gesture.Move(200, snap: false);

        Assert.Null(feedback.Preview);
        Assert.Null(feedback.Guide);
        Assert.Equal(50, section.From);
        Assert.Equal(150, section.To);
    }

    [Fact]
    public void CancellingTwiceChangesNothingFurther()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(50, 150)!;
        var gesture = new BorderSectionGesture(side);

        gesture.Press(100, snap: false);
        gesture.Move(120, snap: false);
        gesture.Cancel();

        Assert.Null(gesture.Cancel());
        Assert.Equal(50, section.From);
        Assert.Equal(150, section.To);
    }

    [Fact]
    public void CancellingAGestureThatNeverStartedDoesNothing()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(50, 150)!;

        Assert.Null(new BorderSectionGesture(side).Cancel());
        Assert.Equal(50, section.From);
        Assert.Equal(150, section.To);
    }

    //==================//
    // Lost capture     //
    //==================//

    // A lost capture cancels, so what matters is exactly when it does NOT: the
    // release itself drops the capture, and the loss that follows must not undo the
    // edit the release just settled.

    [Fact]
    public void TheCaptureLostOnReleaseDoesNotUndoAResize()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(50, 150)!;
        var gesture = new BorderSectionGesture(side);

        gesture.Press(53, snap: false);
        gesture.Move(63, snap: false);
        gesture.Release(63, snap: false);

        Assert.Null(gesture.Cancel());

        Assert.Equal(60, section.From);
        Assert.Equal(150, section.To);
    }

    [Fact]
    public void TheCaptureLostOnReleaseDoesNotUndoADrawing()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var gesture = new BorderSectionGesture(side);

        gesture.Press(50, snap: false);
        gesture.Move(150, snap: false);
        var created = gesture.Release(150, snap: false);

        Assert.Null(gesture.Cancel());

        Assert.NotNull(created);
        Assert.Equal(50, created!.From);
        Assert.Equal(150, created.To);
        Assert.Single(side.Side.Sections.Items);
    }

    //==================//
    // The guide        //
    //==================//

    [Fact]
    public void DrawingMarksTheMovingEndWhenItLandsOnATarget()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var gesture = new BorderSectionGesture(side);

        // 50 is near nothing; the edge's middle at 135 is a target, and 133 is within
        // the 4 mm tolerance of it.
        gesture.Press(50, snap: true);
        var feedback = gesture.Move(133, snap: true);

        Assert.Equal(new BorderSpan(50, 135), feedback.Preview);
        Assert.Equal(new BorderGuideMark(135, SnapKind.Middle), feedback.Guide);
    }

    [Fact]
    public void NothingIsMarkedWhileSnappingIsSuspended()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var gesture = new BorderSectionGesture(side);

        gesture.Press(50, snap: false);
        var feedback = gesture.Move(133, snap: false);

        // Ctrl held: the sweep goes where the pointer is and catches nothing.
        Assert.Equal(new BorderSpan(50, 133), feedback.Preview);
        Assert.Null(feedback.Guide);
    }

    [Fact]
    public void ABoundaryBetweenTargetsIsNotMarked()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var gesture = new BorderSectionGesture(side);

        gesture.Press(50, snap: true);

        Assert.Null(gesture.Move(100, snap: true).Guide);
    }

    [Fact]
    public void ResizingMarksTheBoundaryItMoved()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(50, 200)!;
        var gesture = new BorderSectionGesture(side);

        gesture.Press(53, snap: true);
        var feedback = gesture.Move(136, snap: true);

        Assert.Equal(135, section.From);
        Assert.Equal(new BorderGuideMark(135, SnapKind.Middle), feedback.Guide);
    }

    [Fact]
    public void AMovedSectionMarksWhicheverEndCaughtSomething()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(10, 40)!;
        var gesture = new BorderSectionGesture(side);

        // A moved section keeps its length, so only one of its two ends can land on a
        // target — and which one is not known in advance. Here the far end catches
        // the middle of the edge and the start is left over nothing.
        gesture.Press(25, snap: true);
        var feedback = gesture.Move(118, snap: true);

        Assert.Equal(105, section.From);
        Assert.Equal(135, section.To);
        Assert.Equal(new BorderGuideMark(135, SnapKind.Middle), feedback.Guide);
    }

    [Fact]
    public void AMovedSectionMarksItsStartWhenThatIsTheEndThatCaught()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var section = side.Create(10, 40)!;
        var gesture = new BorderSectionGesture(side);

        gesture.Press(25, snap: true);
        var feedback = gesture.Move(152, snap: true);

        Assert.Equal(135, section.From);
        Assert.Equal(165, section.To);
        Assert.Equal(new BorderGuideMark(135, SnapKind.Middle), feedback.Guide);
    }
}
