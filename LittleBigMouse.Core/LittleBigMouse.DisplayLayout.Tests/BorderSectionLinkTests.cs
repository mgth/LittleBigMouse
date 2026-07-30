using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The link compiler is where border sections actually live: the daemon has no
/// notion of a section, it only ever sees the flat per-side <c>ZoneLink</c> list
/// produced here. These tests lock that translation — the subdivision, the
/// resistances carried by each run, and the fact that the wire keeps the historical
/// <c>BorderResistance</c> attribute name for the move resistance.
/// </summary>
public class BorderSectionLinkTests
{
    // Two 1920x1080 monitors of equal DPI, side by side. Left's right edge runs
    // 0..270 mm and maps 1:1 onto Right's left edge.
    const double EdgeHeightMm = 270;

    static ZonesLayout TwoMonitors(out BorderResistance leftBorders)
    {
        leftBorders = new BorderResistance();

        var layout = new ZonesLayout();
        layout.Zones.Add(new Zone(
            leftBorders, "LEFT", "Left",
            new Rect(-1920, 0, 1920, 1080),
            new Rect(-480, 0, 480, EdgeHeightMm)));
        layout.Zones.Add(new Zone(
            new BorderResistance(), "RIGHT", "Right",
            new Rect(0, 0, 1920, 1080),
            new Rect(0, 0, 480, EdgeHeightMm)));

        return layout;
    }

    static List<ZoneLink> RightLinksOfLeft(ZonesLayout layout)
    {
        layout.Init();
        return layout.Zones.First(z => z.Name == "Left").RightLinks;
    }

    /// <summary>Links that actually cross into the neighbour, in edge order.</summary>
    static List<ZoneLink> Crossing(List<ZoneLink> links) => [.. links.Where(l => l.Target != null)];

    [Fact]
    public void NoSections_LeavesTheEdgeFree()
    {
        // An edge carries no resistance of its own any more: what no section covers
        // is free.
        var layout = TwoMonitors(out _);

        var crossing = Assert.Single(Crossing(RightLinksOfLeft(layout)));

        Assert.Equal(0, crossing.BorderResistance);
        Assert.Equal(0, crossing.DragResistance);
        Assert.False(crossing.MoveBlock);
        Assert.False(crossing.DragBlock);
    }

    [Fact]
    public void ASectionSpanningTheEdgeIsTheFormerPerEdgeResistance()
    {
        // The shape a stored per-edge resistance is converted into on load: one run
        // over the whole edge, exactly as before the notion was dropped.
        var layout = TwoMonitors(out var borders);
        borders.Right.Sections.Add(new BorderSection
        {
            From = 0, To = EdgeHeightMm, Move = 7, Drag = 9
        });

        var crossing = Assert.Single(Crossing(RightLinksOfLeft(layout)));

        Assert.Equal(7, crossing.BorderResistance);
        Assert.Equal(9, crossing.DragResistance);
        Assert.False(crossing.MoveBlock);
    }

    [Fact]
    public void ASectionSplitsTheEdgeAndCarriesItsOwnResistances()
    {
        var layout = TwoMonitors(out var borders);
        // Top half of the edge: a wall for plain moves, heavy for drags. The bottom
        // half is left uncovered, so it stays free.
        borders.Right.Sections.Add(new BorderSection
        {
            From = 0, To = EdgeHeightMm / 2, MoveBlock = true, Drag = 50
        });

        var crossing = Crossing(RightLinksOfLeft(layout));

        Assert.Equal(2, crossing.Count);

        Assert.True(crossing[0].MoveBlock);
        Assert.Equal(50, crossing[0].DragResistance);
        Assert.Equal(0, crossing[0].From);
        Assert.Equal(EdgeHeightMm / 2, crossing[0].To);
        // The cut lands on the matching pixel row, which is what the daemon indexes.
        Assert.Equal(540, crossing[0].SourceToPixel);

        Assert.False(crossing[1].MoveBlock);
        Assert.Equal(0, crossing[1].BorderResistance);
        Assert.Equal(0, crossing[1].DragResistance);
        Assert.Equal(540, crossing[1].SourceFromPixel);
    }

    [Fact]
    public void AdjacentSectionsWithDifferentSettingsAreNotMerged()
    {
        // Regression guard: the merge used to compare only the target zone, so two
        // touching sections pointing at the same monitor collapsed into one and the
        // second one's settings vanished.
        var layout = TwoMonitors(out var borders);
        borders.Right.Sections.Add(new BorderSection { From = 0, To = 90, Move = 10 });
        borders.Right.Sections.Add(new BorderSection { From = 90, To = 180, Move = 20 });

        var crossing = Crossing(RightLinksOfLeft(layout));

        Assert.Equal(3, crossing.Count);
        Assert.Equal(10, crossing[0].BorderResistance);
        Assert.Equal(20, crossing[1].BorderResistance);
        Assert.Equal(0, crossing[2].BorderResistance);
    }

    [Fact]
    public void AdjacentSectionsWithIdenticalSettingsStillMerge()
    {
        var layout = TwoMonitors(out var borders);
        borders.Right.Sections.Add(new BorderSection { From = 0, To = 90, Move = 10 });
        borders.Right.Sections.Add(new BorderSection { From = 90, To = 180, Move = 10 });

        var crossing = Crossing(RightLinksOfLeft(layout));

        Assert.Equal(2, crossing.Count);
        Assert.Equal(10, crossing[0].BorderResistance);
        Assert.Equal(180, crossing[0].To);
    }

    [Fact]
    public void SectionsAreRelativeToTheEdgeStartCorner()
    {
        // Same section, monitor moved down 1000 mm: the cut must follow the monitor,
        // not sit at an absolute layout coordinate.
        var borders = new BorderResistance();
        borders.Right.Sections.Add(new BorderSection { From = 0, To = 135, MoveBlock = true });

        var layout = new ZonesLayout();
        layout.Zones.Add(new Zone(
            borders, "LEFT", "Left",
            new Rect(-1920, 0, 1920, 1080),
            new Rect(-480, 1000, 480, EdgeHeightMm)));
        layout.Zones.Add(new Zone(
            new BorderResistance(), "RIGHT", "Right",
            new Rect(0, 0, 1920, 1080),
            new Rect(0, 1000, 480, EdgeHeightMm)));

        var crossing = Crossing(RightLinksOfLeft(layout));

        Assert.Equal(2, crossing.Count);
        Assert.True(crossing[0].MoveBlock);
        Assert.Equal(1000, crossing[0].From);
        Assert.Equal(1135, crossing[0].To);
        Assert.False(crossing[1].MoveBlock);
    }

    [Fact]
    public void SerializedLinkKeepsTheHistoricalAttributeNames()
    {
        // The daemon reads BorderResistance as the move resistance and falls back to
        // it when DragResistance is absent, so this naming is a compatibility
        // contract with the Rust parser, not a style choice.
        var layout = TwoMonitors(out var borders);
        borders.Right.Sections.Add(new BorderSection
        {
            From = 0, To = EdgeHeightMm, Move = 3, Drag = 4, DragBlock = true
        });

        var xml = Crossing(RightLinksOfLeft(layout))[0].Serialize();

        Assert.Contains(@"BorderResistance=""3""", xml);
        Assert.Contains(@"DragResistance=""4""", xml);
        Assert.Contains(@"MoveBlock=""False""", xml);
        Assert.Contains(@"DragBlock=""True""", xml);
    }
}
