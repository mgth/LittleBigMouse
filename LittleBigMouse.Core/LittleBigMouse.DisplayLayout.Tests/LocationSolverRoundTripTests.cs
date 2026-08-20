using HLab.Geo;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using Xunit.Abstractions;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The two placement directions are inverses of each other, and this is where that claim is
/// checked: read a system pixel configuration into millimetres with
/// <see cref="SystemLocationSolver"/>, push the result back out with
/// <see cref="PixelLocationSolver"/>, and land on the configuration you started from.
/// <para>
/// It only holds where it can. A physical layout cannot express "300 pixels of nothing between
/// these two screens", and a millimetre arrangement carries information no integer pixel grid
/// can carry back; the second half of this file pins down exactly which configurations are lost
/// and what they become, so that a future change to either solver has to say so out loud.
/// </para>
/// <para>
/// Both directions run here as pure functions on <see cref="MonitorSnapshot"/> — no reactive
/// model. <see cref="PlaceFromSystemTests"/> keeps covering the same pair through the live one.
/// </para>
/// </summary>
public class LocationSolverRoundTripTests(ITestOutputHelper output)
{
    /// <summary>
    /// A monitor as a freshly built layout has it: known panel size and bezels, known pixel rect,
    /// and no millimetre position yet — everything stacked at the origin.
    /// </summary>
    static MonitorSnapshot Monitor(
        string id, double widthMm, double heightMm,
        double pixelX, double pixelY, double pixelWidth, double pixelHeight,
        bool primary = false, double bezel = 10)
        => new(
            id,
            new Rect(0, 0, widthMm, heightMm),
            new Rect(-bezel, -bezel, widthMm + 2 * bezel, heightMm + 2 * bezel),
            new Rect(pixelX, pixelY, pixelWidth, pixelHeight),
            primary);

    /// <summary>
    /// "Place from system" end to end, as <see cref="MonitorsLocationsFromSystemExtensions.SetLocationsFromSystemConfiguration"/>
    /// runs it: solve the walk, then compact — monitors the walk could not claim only get their
    /// final position from <see cref="CompactionSolver"/>, and that second step is where the
    /// information a round trip loses actually gets lost.
    /// </summary>
    static IReadOnlyList<MonitorSnapshot> PlaceFromSystem(
        IReadOnlyList<MonitorSnapshot> monitors, double minimalEdgeOverlap = 20)
    {
        var solved = SystemLocationSolver.Solve(monitors);
        var placed = monitors
            .Select(m => solved.TryGetValue(m.Id, out var p) ? m.MovedTo(p) : m)
            .ToList();

        var offsets = CompactionSolver.Solve(
            [.. placed.Select(m => m.ForCompaction())],
            new CompactionOptions(AllowOverlaps: false, AllowDiscontinuity: false,
                MinimalEdgeOverlap: minimalEdgeOverlap));

        return [.. placed.Select(m => offsets.TryGetValue(m.Id, out var v)
            ? m.MovedTo(new Point(m.MmBounds.X + v.X, m.MmBounds.Y + v.Y))
            : m)];
    }

    /// <summary>Pixel positions relative to the primary — the anchor both directions agree on.</summary>
    static Dictionary<string, Point> SystemPixels(IReadOnlyList<MonitorSnapshot> monitors)
    {
        var origin = monitors.Single(m => m.Primary).PixelBounds.Location;
        return monitors.ToDictionary(m => m.Id,
            m => new Point(m.PixelBounds.X - origin.X, m.PixelBounds.Y - origin.Y));
    }

    string Describe(IReadOnlyList<MonitorSnapshot> placed, IReadOnlyDictionary<string, Point> back)
        => string.Join(" | ", placed.OrderBy(m => m.Id).Select(m =>
            $"{m.Id} mm({m.MmBounds.X:F1},{m.MmBounds.Y:F1}) -> px({back[m.Id].X},{back[m.Id].Y})"));

    /// <summary>
    /// System pixels in, the same system pixels out. One pixel of slack: the outbound direction
    /// rounds, because the system only accepts integers.
    /// </summary>
    void AssertRoundTrips(IReadOnlyList<MonitorSnapshot> monitors)
    {
        var placed = PlaceFromSystem(monitors);
        var back = PixelLocationSolver.Solve(placed);
        var expected = SystemPixels(monitors);

        output.WriteLine(Describe(placed, back));

        foreach (var monitor in monitors)
        {
            var want = expected[monitor.Id];
            var got = back[monitor.Id];

            Assert.True(Math.Abs(got.X - want.X) <= 1,
                $"{monitor.Id} X: got {got.X}, expected {want.X}");
            Assert.True(Math.Abs(got.Y - want.Y) <= 1,
                $"{monitor.Id} Y: got {got.Y}, expected {want.Y}");
        }
    }

    // ---------------------------------------------------------------- invertible

    [Fact]
    public void SideBySide_RoundTrips()
    {
        AssertRoundTrips([
            Monitor("A", 600, 340, 0, 0, 2560, 1440, primary: true),
            Monitor("B", 600, 340, 2560, 0, 2560, 1440),
        ]);
    }

    /// <summary>
    /// The desktop this whole pair exists for: different panel sizes, wildly different pitches,
    /// and a neighbour hanging 219 px lower. Nobody bolts their screens to a shelf.
    /// </summary>
    [Fact]
    public void MixedPitchesAndVerticalOffset_RoundTrips()
    {
        AssertRoundTrips([
            Monitor("MAIN", 600, 340, 0, 0, 2560, 1440, primary: true),
            Monitor("SIDE", 600, 340, 2560, -219, 2560, 1440),
            Monitor("TV", 1650, 920, 5120, 0, 1280, 720),
        ]);
    }

    [Fact]
    public void VerticalStackOfDifferentWidths_RoundTrips()
    {
        AssertRoundTrips([
            Monitor("TOP", 600, 340, 0, 0, 2560, 1440, primary: true),
            Monitor("LAPTOP", 302, 189, 320, 1440, 1920, 1200),
        ]);
    }

    [Fact]
    public void PrimaryInTheMiddle_RoundTrips()
    {
        AssertRoundTrips([
            Monitor("LEFT", 520, 320, -1920, 0, 1920, 1080),
            Monitor("MAIN", 600, 340, 0, 0, 2560, 1440, primary: true),
            Monitor("RIGHT", 700, 400, 2560, 0, 1920, 1080),
        ]);
    }

    /// <summary>
    /// Bezel widths are the one thing the system never sees. They must survive the outbound trip
    /// anyway — as exact pixel contact, since the system has nowhere to put them.
    /// </summary>
    [Fact]
    public void ThickBezels_RoundTripAsExactPixelContact()
    {
        var monitors = new[]
        {
            Monitor("A", 600, 340, 0, 0, 1920, 1080, primary: true, bezel: 25),
            Monitor("B", 600, 340, 1920, 0, 1920, 1080, bezel: 25),
        };

        AssertRoundTrips(monitors);

        var placed = PlaceFromSystem(monitors);
        var a = placed.Single(m => m.Id == "A");
        var b = placed.Single(m => m.Id == "B");

        // 50mm of frame between the two panels, and the bezels themselves flush.
        Assert.Equal(50, b.MmBounds.X - a.MmBounds.Right, 6);
        Assert.Equal(a.MmOutsideBounds.Right, b.MmOutsideBounds.X, 6);
    }

    /// <summary>
    /// Running "place from system" twice must give what running it once gave. Both entry points
    /// (first launch, and the menu item) call the same thing, so a result that depends on where
    /// the monitors already were means the button and the first launch disagree.
    /// </summary>
    [Fact]
    public void PlacingIsIdempotent()
    {
        var monitors = new[]
        {
            Monitor("MAIN", 600, 340, 0, 0, 2560, 1440, primary: true),
            Monitor("SIDE", 600, 340, 2560, -219, 2560, 1440),
            Monitor("TV", 1650, 920, 5120, 0, 1280, 720),
        };

        var once = PlaceFromSystem(monitors);
        var twice = PlaceFromSystem(once);

        foreach (var monitor in once)
        {
            var again = twice.Single(m => m.Id == monitor.Id);
            Assert.Equal(monitor.MmBounds.X, again.MmBounds.X, 9);
            Assert.Equal(monitor.MmBounds.Y, again.MmBounds.Y, 9);
        }
    }

    /// <summary>
    /// A 2x2 grid, which is where the diagonal neighbour has to be refused. D shares a pixel edge
    /// with the primary on BOTH axes at once — a corner, which the cursor cannot cross — and a
    /// contact with no overlap across it does not claim. So D is not consumed on the diagonal: it
    /// waits for B (or C), whose edge with it is real, and docks bezel-flush like everything else.
    /// Nothing overlaps, compaction finds nothing to do, and the grid comes back as a grid.
    /// <para>
    /// This used to be the one configuration in this file that did not survive the trip: both
    /// corner contacts fired, each ran the perpendicular projection on the other axis, and the two
    /// projections overwrote both coordinates the contact rules had set. D landed panel-flush at
    /// (600,340) with its frame inside the primary's, and compaction skewed the whole grid by a
    /// bezel width on each axis to pull them apart — B to (620,-20), C to (-20,360).
    /// </para>
    /// </summary>
    [Fact]
    public void DiagonalNeighbourInAGrid_IsRefused_AndTheGridRoundTrips()
    {
        var monitors = new[]
        {
            Monitor("A", 600, 340, 0, 0, 1920, 1080, primary: true),
            Monitor("B", 600, 340, 1920, 0, 1920, 1080),
            Monitor("C", 600, 340, 0, 1080, 1920, 1080),
            Monitor("D", 600, 340, 1920, 1080, 1920, 1080),
        };

        // The walk alone, before compaction: every monitor bezel-flush against its neighbours,
        // D at 620 rather than the 600 a panel-flush diagonal dock used to give.
        var walked = SystemLocationSolver.Solve(monitors);
        Assert.Equal(new Point(620, 0), walked["B"]);
        Assert.Equal(new Point(0, 360), walked["C"]);
        Assert.Equal(new Point(620, 360), walked["D"]);

        // Bezels touching on all four inner edges, so compaction has nothing left to separate and
        // the grid keeps its shape.
        var placed = PlaceFromSystem(monitors);
        Assert.Equal(new Point(620, 0), placed.Single(m => m.Id == "B").MmLocation);
        Assert.Equal(new Point(0, 360), placed.Single(m => m.Id == "C").MmLocation);
        Assert.Equal(new Point(620, 360), placed.Single(m => m.Id == "D").MmLocation);

        AssertRoundTrips(monitors);
    }

    // ------------------------------------------------------------ not invertible

    /// <summary>
    /// A gap between two monitors in the system configuration has no physical meaning: the
    /// millimetre layout is a description of where the panels ARE, and there is no way to say
    /// "and 300 pixels of nothing here". The gap is closed on the way in and cannot come back.
    /// </summary>
    [Fact]
    public void PixelGap_IsClosed_AndDoesNotComeBack()
    {
        var monitors = new[]
        {
            Monitor("A", 600, 340, 0, 0, 1920, 1080, primary: true),
            Monitor("B", 600, 340, 1920 + 300, 0, 1920, 1080),
        };

        var placed = PlaceFromSystem(monitors);
        var back = PixelLocationSolver.Solve(placed);
        output.WriteLine(Describe(placed, back));

        // Not where it was: the gap is gone.
        Assert.NotEqual(2220, back["B"].X);

        // What it becomes is worth being explicit about. No pixel edge is shared, so the walk
        // never claims B — only the "same top edge, same bottom edge" alignment hints fire, and
        // those leave it exactly on top of the primary. Compaction then has a total overlap to
        // break and picks the shortest push, which on two identical panels is vertical. Two
        // screens the user has side by side come back stacked.
        Assert.Equal(new Point(0, 1080), back["B"]);

        // Whatever it chose, the desktop is connected and free of overlap, which is the part
        // both platforms actually require.
        var a = new Rect(back["A"], monitors[0].PixelSize);
        var b = new Rect(back["B"], monitors[1].PixelSize);
        Assert.False(LayoutGeometry.Overlap(a, b));
        Assert.Equal(a.Bottom, b.Y);
    }

    /// <summary>
    /// Two monitors overlapping in the system configuration: same story the other way. The
    /// physical layout cannot describe two panels occupying the same space, so the overlap is
    /// resolved on the way in and the round trip returns a layout that merely touches.
    /// </summary>
    [Fact]
    public void PixelOverlap_IsResolved_AndDoesNotComeBack()
    {
        var monitors = new[]
        {
            Monitor("A", 600, 340, 0, 0, 1920, 1080, primary: true),
            Monitor("B", 600, 340, 960, 540, 1920, 1080),
        };

        var placed = PlaceFromSystem(monitors);
        var back = PixelLocationSolver.Solve(placed);
        output.WriteLine(Describe(placed, back));

        Assert.NotEqual(new Point(960, 540), back["B"]);

        var a = new Rect(back["A"], monitors[0].PixelSize);
        var b = new Rect(back["B"], monitors[1].PixelSize);
        Assert.False(LayoutGeometry.Overlap(a, b), $"A{a} still overlaps B{b}");
    }

    /// <summary>
    /// Two monitors meeting corner to corner. A corner is not crossable — the cursor has nowhere
    /// to pass — so nothing downstream is willing to keep it: compaction slides the monitor until
    /// the panels share the 20mm corridor the layout options ask for, and the outbound direction
    /// only ever docks along an edge with real overlap. The diagonal comes back as a side by
    /// side, on purpose.
    /// </summary>
    [Fact]
    public void CornerToCornerContact_IsNotInvertible()
    {
        var monitors = new[]
        {
            Monitor("A", 600, 340, 0, 0, 1920, 1080, primary: true),
            Monitor("B", 600, 340, 1920, 1080, 1920, 1080),
        };

        // Inbound, the walk refuses the contact on both axes — neither has any overlap across it
        // — so B is never claimed and no projection runs on it. The contact rule still SUGGESTS a
        // position, which is the whole difference between a corner-only neighbour and one that
        // touches nothing: B keeps the diagonal it was on, bezel-flush at (620,360) rather than
        // staying stacked on the primary at the origin.
        var walked = SystemLocationSolver.Solve(monitors);
        Assert.Equal(new Point(620, 360), walked["B"]);

        var placed = PlaceFromSystem(monitors);
        var back = PixelLocationSolver.Solve(placed);
        output.WriteLine(Describe(placed, back));

        var a = placed.Single(m => m.Id == "A");
        var b = placed.Single(m => m.Id == "B");

        // Compaction then keeps the frames flush on the right and slides B up until the two
        // panels share the 20mm corridor the options demand.
        Assert.Equal(a.MmOutsideBounds.Right, b.MmOutsideBounds.X, 6);
        Assert.Equal(20, LayoutGeometry.OverlapOn(a.MmBounds, b.MmBounds, Axis.Vertical), 6);

        // Outbound: not a corner any more. B is docked on one axis with a real shared edge.
        var pa = new Rect(back["A"], monitors[0].PixelSize);
        var pb = new Rect(back["B"], monitors[1].PixelSize);
        var sharedEdge =
            (pa.Right == pb.X || pb.Right == pa.X) && LayoutGeometry.OverlapOn(pa, pb, Axis.Vertical) > 0
            || (pa.Bottom == pb.Y || pb.Bottom == pa.Y) && LayoutGeometry.OverlapOn(pa, pb, Axis.Horizontal) > 0;

        Assert.True(sharedEdge, $"corner became {pa} / {pb}, still not crossable");
        Assert.NotEqual(new Point(1920, 1080), back["B"]);
    }

    /// <summary>
    /// The same corner, for the user who asked to keep corners: <c>MinimalEdgeOverlap = 0</c> is
    /// how bare corner contact is accepted, and with the walk leaving B bezel-flush on the
    /// diagonal there is nothing for compaction to undo. The physical layout the user sees is
    /// then the one the system reported, which is the payoff for a refused contact still
    /// SUGGESTING a position: were B left unplaced it would sit on the primary and be shoved off
    /// to one side instead.
    /// <para>
    /// Only the millimetre half survives — the trip back out still cannot express it, since
    /// <see cref="PixelLocationSolver"/> refuses corners whatever the options say and treats B as
    /// a detached island to be snapped into contact. Two solvers, one option: the corridor rule
    /// is layout editing, the pixel grid has to stay crossable regardless.
    /// </para>
    /// </summary>
    [Fact]
    public void CornerToCornerContact_SurvivesWhenNoCorridorIsDemanded()
    {
        var monitors = new[]
        {
            Monitor("A", 600, 340, 0, 0, 1920, 1080, primary: true),
            Monitor("B", 600, 340, 1920, 1080, 1920, 1080),
        };

        var placed = PlaceFromSystem(monitors, minimalEdgeOverlap: 0);
        var back = PixelLocationSolver.Solve(placed);
        output.WriteLine(Describe(placed, back));

        var a = placed.Single(m => m.Id == "A");
        var b = placed.Single(m => m.Id == "B");

        // Frames meeting at the single point (610, 350), panels still 20mm apart on each axis.
        Assert.Equal(new Point(620, 360), b.MmLocation);
        Assert.Equal(a.MmOutsideBounds.Right, b.MmOutsideBounds.X, 6);
        Assert.Equal(a.MmOutsideBounds.Bottom, b.MmOutsideBounds.Y, 6);
    }

    /// <summary>
    /// A monitor touching nothing in the system configuration is placed by compaction, not by the
    /// walk — its position comes from "dock it nearest first", not from the pixel configuration,
    /// so there is nothing to invert. It must nevertheless end up part of the single connected
    /// block both platforms require.
    /// </summary>
    [Fact]
    public void DetachedMonitor_IsDockedRatherThanInverted()
    {
        var monitors = new[]
        {
            Monitor("MAIN", 600, 340, 0, 0, 2560, 1440, primary: true),
            Monitor("FAR", 520, 300, 6000, 4000, 1920, 1080),
        };

        var placed = PlaceFromSystem(monitors);
        var back = PixelLocationSolver.Solve(placed);
        output.WriteLine(Describe(placed, back));

        Assert.NotEqual(new Point(6000, 4000), back["FAR"]);

        var main = new Rect(back["MAIN"], monitors[0].PixelSize);
        var far = new Rect(back["FAR"], monitors[1].PixelSize);
        Assert.False(LayoutGeometry.Overlap(main, far));

        var touching =
            (main.Right == far.X || far.Right == main.X) && LayoutGeometry.OverlapOn(main, far, Axis.Vertical) > 0
            || (main.Bottom == far.Y || far.Bottom == main.Y) && LayoutGeometry.OverlapOn(main, far, Axis.Horizontal) > 0;

        Assert.True(touching, $"island left disconnected: {main} / {far}");
    }

    /// <summary>
    /// A monitor reporting no pixels has no pitch, so the shared-midpoint projection cannot run
    /// for it. It must not produce a NaN position, and it must not take the rest of the layout
    /// down with it — the walk goes through half-built states on every layout rebuild.
    /// </summary>
    [Fact]
    public void MonitorWithoutPixels_IsPlacedWithoutProjection()
    {
        var monitors = new[]
        {
            Monitor("MAIN", 600, 340, 0, 0, 2560, 1440, primary: true),
            Monitor("GHOST", 520, 300, 2560, 0, 0, 0),
        };

        var placed = PlaceFromSystem(monitors);
        output.WriteLine(string.Join(" | ", placed.Select(m => $"{m.Id}({m.MmBounds.X:F1},{m.MmBounds.Y:F1})")));

        foreach (var monitor in placed)
        {
            Assert.False(double.IsNaN(monitor.MmBounds.X), $"{monitor.Id} X is NaN");
            Assert.False(double.IsNaN(monitor.MmBounds.Y), $"{monitor.Id} Y is NaN");
        }
    }

    /// <summary>
    /// No primary yet — the state every layout passes through while it is being built. Nothing is
    /// anchored, so nothing may move: a walk with no root would otherwise pick an arbitrary
    /// monitor and scatter the layout around it.
    /// </summary>
    [Fact]
    public void NoPrimary_MovesNothing()
    {
        var solved = SystemLocationSolver.Solve([
            Monitor("A", 600, 340, 0, 0, 2560, 1440),
            Monitor("B", 520, 300, 2560, 0, 1920, 1080),
        ]);

        Assert.Empty(solved);
    }

    /// <summary>
    /// Monitors already placed are excluded from the walk: they neither move nor anchor anything.
    /// This is the <c>placeAll: false</c> path the platform factories take on startup, so a saved
    /// layout is not silently re-derived from the system.
    /// </summary>
    [Fact]
    public void MonitorsNotOfferedForPlacement_StayPut()
    {
        var monitors = new[]
        {
            Monitor("MAIN", 600, 340, 0, 0, 2560, 1440, primary: true),
            Monitor("SIDE", 600, 340, 2560, -219, 2560, 1440),
        };

        var solved = SystemLocationSolver.Solve(monitors, new HashSet<string>());
        Assert.Empty(solved);

        // Offered, and then it does move.
        Assert.True(SystemLocationSolver.Solve(monitors, new HashSet<string> { "SIDE" }).ContainsKey("SIDE"));
    }
}
