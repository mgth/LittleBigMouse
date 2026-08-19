using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Reactive;
using System.Reactive.Disposables;
using System.Reactive.Linq;
using System.Reactive.Subjects;
using DynamicData;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>What a press on a band takes hold of.</summary>
public enum BorderGrab
{
    /// <summary>Nothing: the press landed in free space and starts a new section.</summary>
    None,

    /// <summary>The section's starting boundary.</summary>
    ResizeFrom,

    /// <summary>The section's ending boundary.</summary>
    ResizeTo,

    /// <summary>The section itself, to slide it along the edge.</summary>
    Move
}

/// <summary>
/// The editing surface for one monitor edge.
/// <para>
/// Everything is expressed in millimetres in the model and in UI pixels in the
/// view; this view model owns the single conversion between them. The overlay this
/// strip lives in covers the monitor's VISIBLE area (MonitorFrameView's centre
/// cell), which is also the rectangle <c>Zone.PhysicalBounds</c> is built from — so
/// a pixel offset here means the same thing to the daemon.
/// </para>
/// </summary>
public class BorderSideViewModel : ReactiveObject, IDisposable
{
    /// <summary>A section thinner than this is a mis-click, not an intent.</summary>
    public const double MinimumLengthMm = 2.0;

    /// <summary>
    /// The link compiler walks the per-side link list linearly, which is ideal for a
    /// handful of entries. This keeps a pathological layout out of the hot path.
    /// </summary>
    public const int MaximumSections = 16;

    const double SnapToleranceUiPixels = 8.0;

    /// <summary>The smallest a handle is ever allowed to get, in UI pixels.</summary>
    const double MinimumHandleUiPixels = 6.0;

    /// <summary>
    /// How deep a handle reaches along the edge, in UI pixels.
    /// <para>
    /// Half the band's thickness rather than a fixed count: the bands on the real
    /// screens are thicker than the one on the layout map, and they are aimed at with
    /// a pointer on glass rather than on a zoomed miniature, so the same handful of
    /// pixels that is comfortable on the map is a sliver out there.
    /// </para>
    /// </summary>
    public double HandleUiLength => System.Math.Max(MinimumHandleUiPixels, PixelThickness / 2);

    public BorderSideViewModel(PhysicalMonitor monitor, BorderSideKind kind, BorderSide side)
    {
        Monitor = monitor;
        Kind = kind;
        Side = side;

        // AutoRefresh so that editing a section in place — not just adding or
        // removing one — reaches the "is this edge sealed" warning too.
        _subscriptions.Add(side.Sections.Connect()
            .AutoRefresh(e => e.MoveBlock)
            .AutoRefresh(e => e.DragBlock)
            .AutoRefresh(e => e.From)
            .AutoRefresh(e => e.To)
            .ToCollection()
            .Subscribe(OnSectionsChanged));

        // Thickness as well as length: the handles are sized from it, so a band that
        // is told how thick it is after its sections were projected would draw them
        // with the handles of the size it was born with.
        _subscriptions.Add(this.WhenAnyValue(e => e.PixelLength, e => e.PixelThickness)
            .Subscribe(_ =>
            {
                foreach (var section in Sections) section.Refresh();
            }));

        // Whether a section can be mirrored depends on where the monitors sit
        // relative to one another, so it has to be re-answered whenever any of them
        // moves or is resized — not only when this edge is edited.
        foreach (var other in monitor.Layout.PhysicalMonitors)
        {
            _subscriptions.Add(other.DepthProjection
                .WhenAnyValue(d => d.X, d => d.Y, d => d.Width, d => d.Height)
                .Skip(1)
                .Subscribe(_ => LayoutGeometryChanged.OnNext(Unit.Default)));
        }
    }

    /// <summary>Fires when any monitor moves or changes size.</summary>
    public Subject<Unit> LayoutGeometryChanged { get; } = new();

    readonly CompositeDisposable _subscriptions = [];

    /// <summary>
    /// One side view model exists per view of the same edge — the layout map plus
    /// one per screen band — and the on-screen ones are created and dropped every
    /// time the overlay is toggled, so their subscriptions to the shared section
    /// list have to go with them.
    /// </summary>
    public void Dispose()
    {
        _subscriptions.Dispose();
        LayoutGeometryChanged.Dispose();
        foreach (var section in Sections) section.Dispose();
        Sections.Clear();
    }

    public PhysicalMonitor Monitor { get; }

    public BorderSideKind Kind { get; }

    public BorderSide Side { get; }

    public ObservableCollection<BorderSectionViewModel> Sections { get; } = [];

    /// <summary>Left and Right edges run along Y; Top and Bottom along X.</summary>
    public bool IsVertical => IsVerticalKind(Kind);

    static bool IsVerticalKind(BorderSideKind kind) =>
        kind is BorderSideKind.Left or BorderSideKind.Right;

    // Taken on any monitor and edge, not just this one: the mirror has to measure the
    // edge it is copying onto without building a view model for it.
    static double OriginOf(PhysicalMonitor monitor, BorderSideKind kind) => IsVerticalKind(kind)
        ? monitor.DepthProjection.Y
        : monitor.DepthProjection.X;

    static double LengthOf(PhysicalMonitor monitor, BorderSideKind kind) => IsVerticalKind(kind)
        ? monitor.DepthProjection.Height
        : monitor.DepthProjection.Width;

    static BorderRectangle BoundsOf(PhysicalMonitor monitor)
    {
        var bounds = monitor.DepthProjection.Bounds;
        return new BorderRectangle(bounds.Left, bounds.Top, bounds.Right, bounds.Bottom);
    }

    IEnumerable<BorderSpan> SectionSpans(BorderSection? excluding = null) =>
        Side.Sections.Items
            .Where(section => !ReferenceEquals(section, excluding))
            .Select(section => new BorderSpan(section.From, section.To));

    /// <summary>Length of the strip in UI pixels, fed by the view on every resize.</summary>
    public double PixelLength
    {
        get;
        set => this.RaiseAndSetIfChanged(ref field, value);
    }

    /// <summary>Across-the-edge thickness of the strip in UI pixels.</summary>
    public double PixelThickness
    {
        get;
        set => this.RaiseAndSetIfChanged(ref field, value);
    } = 16;

    /// <summary>Length of the edge in millimetres.</summary>
    public double LengthMm => IsVertical
        ? Monitor.DepthProjection.Height
        : Monitor.DepthProjection.Width;

    /// <summary>
    /// Absolute layout coordinate of the edge's starting corner — the top for a
    /// vertical edge, the left for a horizontal one. Section coordinates are
    /// relative to it, which is what makes them survive moving the monitor.
    /// </summary>
    public double OriginMm => IsVertical
        ? Monitor.DepthProjection.Y
        : Monitor.DepthProjection.X;

    public double ToPixels(double mm) => LengthMm > 0 ? mm * PixelLength / LengthMm : 0;

    public double ToMm(double pixels) => PixelLength > 0 ? pixels * LengthMm / PixelLength : 0;

    /// <summary>
    /// True when nothing can cross this edge in either mode, over its whole length.
    /// The Ctrl bypass still applies on Windows and X11, but NOT on Wayland, where
    /// modifiers are not observable — so this is a real trap worth flagging.
    /// </summary>
    public bool IsFullyBlocked
    {
        get
        {
            // Blocking sections have to cover the edge end to end, with no gap:
            // anywhere they leave uncovered is free.
            var covered = 0.0;
            foreach (var s in Ordered())
            {
                if (!s.MoveBlock || !s.DragBlock) return false;
                if (s.From > covered + 0.001) return false;
                covered = System.Math.Max(covered, s.To);
            }

            return covered >= LengthMm - 0.001;
        }
    }

    IEnumerable<BorderSection> Ordered() => Side.Sections.Items.OrderBy(s => s.From);

    void OnSectionsChanged(IReadOnlyCollection<BorderSection> sections)
    {
        // Rebuild only when the set of sections actually changed. An in-place edit
        // also lands here through AutoRefresh, and recreating the item view models
        // then would drop the selection and restart any drag in progress.
        if (!Sections.Select(vm => vm.Model).SequenceEqual(sections.OrderBy(s => s.From)))
        {
            // Dropped item view models stay subscribed to the shared selection
            // otherwise, and would keep answering for sections that no longer exist.
            foreach (var vm in Sections) vm.Dispose();

            Sections.Clear();
            foreach (var section in Ordered())
                Sections.Add(new BorderSectionViewModel(section, this));
        }

        this.RaisePropertyChanged(nameof(IsFullyBlocked));
    }

    //==================//
    // Snapping         //
    //==================//

    /// <summary>
    /// Where a section boundary wants to land, in millimetres from the edge origin.
    /// <para>
    /// The neighbours contribute their INNER edges — <c>DepthProjection.Bounds</c>,
    /// the visible area, not <c>OutsideBounds</c> which includes the bezel. That is
    /// deliberate: the daemon's zones are built from the visible rectangle, and
    /// these are exactly the cut points <c>Zone.ComputeLinks</c> already derives, so
    /// snapping onto them guarantees a section never leaves a one-millimetre sliver
    /// of a link behind.
    /// </para>
    /// <para>
    /// Sections drawn anywhere on an edge parallel to this one contribute too, near
    /// or far, so a passage can be lined up with the one across the gap, with the one
    /// on the opposite side of this same screen, or with a screen further along.
    /// </para>
    /// </summary>
    public IReadOnlyList<SnapTarget> SnapTargetsMm(BorderSection? excluding = null)
    {
        var screens = Monitor.Layout.PhysicalMonitors
            .Where(other => !ReferenceEquals(other, Monitor))
            .Select(BoundsOf);

        return BorderSnapEngine.BuildTargets(
            LengthMm,
            OriginMm,
            IsVertical,
            screens,
            SectionSpans(excluding),
            ParallelSectionSpans());
    }

    IEnumerable<ParallelBorderSpan> ParallelSectionSpans()
    {
        var parallel = IsVertical
            ? (BorderSideKind[])[BorderSideKind.Left, BorderSideKind.Right]
            : [BorderSideKind.Top, BorderSideKind.Bottom];

        foreach (var monitor in Monitor.Layout.PhysicalMonitors)
        foreach (var kind in parallel)
        {
            if (ReferenceEquals(monitor, Monitor) && kind == Kind) continue;

            var origin = OriginOf(monitor, kind);
            foreach (var section in SideOf(monitor, kind).Sections.Items)
                yield return new ParallelBorderSpan(origin, new BorderSpan(section.From, section.To));
        }
    }

    /// <summary>The target a boundary has landed exactly on, if any.</summary>
    public SnapTarget? MatchedTarget(double mm, BorderSection? excluding = null) =>
        BorderSnapEngine.MatchedTarget(SnapTargetsMm(excluding), mm);

    /// <summary>
    /// Snap <paramref name="mm"/> to the nearest target within tolerance. The section
    /// being dragged is left out of the targets, or its own boundaries would hold it
    /// where it already is.
    /// </summary>
    public double Snap(double mm, bool enabled = true, BorderSection? excluding = null) =>
        BorderSnapEngine.Snap(
            mm, LengthMm, ToMm(SnapToleranceUiPixels), SnapTargetsMm(excluding), enabled);

    //==================//
    // Editing          //
    //==================//

    /// <summary>
    /// The widest stretch of edge around <paramref name="referenceMm"/> that no other
    /// section occupies. Returns an empty range when the reference itself falls
    /// inside one.
    /// <para>
    /// Sections must never overlap, and this is not cosmetic: the link compiler
    /// resolves an interval by taking the FIRST section containing its midpoint, so
    /// an overlap would make one of them silently win.
    /// </para>
    /// </summary>
    public (double Low, double High) FreeGapAround(double referenceMm, BorderSection? excluding)
    {
        var gap = BorderSectionGeometry.FreeGapAround(SectionSpans(excluding), referenceMm, LengthMm);
        return (gap.From, gap.To);
    }

    /// <summary>Restrict [from, to] to the free gap containing <paramref name="referenceMm"/>.</summary>
    public (double From, double To) ClampToFreeSpace(double from, double to, double referenceMm, BorderSection? excluding)
    {
        var gap = BorderSectionGeometry.FreeGapAround(SectionSpans(excluding), referenceMm, LengthMm);
        var result = BorderSectionGeometry.ClampToFreeSpace(new BorderSpan(from, to), gap);
        return (result.From, result.To);
    }

    /// <summary>
    /// How far from a boundary a press still grabs it, in millimetres.
    /// <para>
    /// Constant along most of the edge, but not in the mitred corners: there the band
    /// tapers to a point, so a fixed handle is a sliver a few pixels wide and all but
    /// unclickable — which is what made the outer ends of a full edge so hard to
    /// resize. Inside a mitre the handle widens inward, towards where the band
    /// recovers its full thickness.
    /// </para>
    /// <para>
    /// Never more than a third of the section: the two handles are drawn where they
    /// answer, so at half each they would meet and cover a short section entirely —
    /// hiding the colour that says what it does, and leaving nowhere to grab to move
    /// it rather than resize it.
    /// </para>
    /// </summary>
    public double GrabMm(double boundaryMm, double sectionLengthMm)
    {
        var basic = ToMm(HandleUiLength);
        var mitre = ToMm(PixelThickness);

        var fromEnd = System.Math.Min(boundaryMm, LengthMm - boundaryMm);
        var grab = fromEnd < mitre ? System.Math.Max(basic, mitre - fromEnd) : basic;

        return System.Math.Min(grab, sectionLengthMm / 3);
    }

    /// <summary>
    /// What a press at <paramref name="mm"/> takes hold of.
    /// <para>
    /// A section that actually holds the point owns the press; the handle margins only
    /// extend a section's reach into the free space AROUND it. That order is the whole
    /// point: two sections that touch both claim their shared boundary, and taking the
    /// first claimant handed every press there to the one starting further up the
    /// edge. Its neighbour's near handle was then unreachable — the boundary could
    /// only ever be moved from one side, and a click just inside the lower section
    /// selected the upper one.
    /// </para>
    /// <para>
    /// Containment is half open, <c>[From, To)</c>, the same convention the link
    /// compiler uses to resolve an interval, so a shared boundary belongs to the
    /// section starting there and each side keeps a handle of its own.
    /// </para>
    /// </summary>
    public (BorderSection? Section, BorderGrab Grab) GrabAt(double mm)
    {
        var section = Ordered().FirstOrDefault(s => mm >= s.From && mm < s.To)
                      ?? NearestWithinHandle(mm);

        if (section == null) return (null, BorderGrab.None);

        var length = section.To - section.From;

        if (mm <= section.From + GrabMm(section.From, length)) return (section, BorderGrab.ResizeFrom);
        if (mm >= section.To - GrabMm(section.To, length)) return (section, BorderGrab.ResizeTo);

        return (section, BorderGrab.Move);
    }

    /// <summary>
    /// The section whose nearest boundary is within grabbing distance of a point that
    /// no section contains, closest first — a press in the free space beside a section
    /// still reaches its handle.
    /// </summary>
    BorderSection? NearestWithinHandle(double mm)
    {
        BorderSection? best = null;
        var bestDistance = double.MaxValue;

        foreach (var section in Ordered())
        {
            var before = mm < section.From;
            var boundary = before ? section.From : section.To;
            var distance = System.Math.Abs(mm - boundary);

            if (distance > GrabMm(boundary, section.To - section.From)) continue;
            if (distance >= bestDistance) continue;

            bestDistance = distance;
            best = section;
        }

        return best;
    }

    /// <summary>
    /// Draw a section. <paramref name="anchorMm"/> is where the gesture started —
    /// necessarily in free space, since pressing on an existing section resizes or
    /// moves it instead. Sweeping past a neighbour therefore stops at its edge
    /// rather than being rejected.
    /// </summary>
    public BorderSection? Create(double anchorMm, double toMm)
    {
        if (Side.Sections.Count >= MaximumSections) return null;

        if (BorderSectionGeometry.Create(
                SectionSpans(), anchorMm, toMm, LengthMm, MinimumLengthMm) is not { } span)
            return null;

        var section = new BorderSection
        {
            From = span.From,
            To = span.To,
            // A fresh section starts as a wall for plain moves and free for drags:
            // that is the #458 shape (a barrier you can still drag a window across),
            // and it makes the section visible the moment it is drawn.
            MoveBlock = true
        };

        Side.Sections.Add(section);
        return section;
    }

    /// <summary>
    /// Grow a section until it meets its neighbours, or the ends of the edge.
    /// </summary>
    public void Expand(BorderSection section)
    {
        if (BorderSectionGeometry.Expand(
                SectionSpans(section),
                new BorderSpan(section.From, section.To),
                LengthMm,
                MinimumLengthMm) is not { } span)
            return;

        section.From = span.From;
        section.To = span.To;
    }

    /// <summary>
    /// Draw a section over the whole free stretch around <paramref name="atMm"/>.
    /// <para>
    /// Same operation as <see cref="Expand"/>, on space that holds nothing yet: both
    /// answer "take everything that is free here", which is what a double click on a
    /// band means whether or not it lands on a section.
    /// </para>
    /// </summary>
    public BorderSection? CreateFilling(double atMm)
    {
        if (Side.Sections.Count >= MaximumSections) return null;
        if (BorderSectionGeometry.CreateFilling(
                SectionSpans(), atMm, LengthMm, MinimumLengthMm) is not { } span)
            return null;

        var section = new BorderSection
        {
            From = span.From,
            To = span.To,
            MoveBlock = true
        };

        Side.Sections.Add(section);
        return section;
    }

    public void Resize(BorderSection section, double fromMm, double toMm)
    {
        if (BorderSectionGeometry.Resize(
                SectionSpans(section),
                new BorderSpan(section.From, section.To),
                fromMm,
                toMm,
                LengthMm,
                MinimumLengthMm) is not { } span)
            return;

        section.From = span.From;
        section.To = span.To;
        // No refresh here: every view of this section follows the model itself.
    }

    /// <summary>
    /// Slide a section along its edge, keeping its length. Pushed into a neighbour it
    /// comes to rest against it rather than refusing to move.
    /// </summary>
    public void MoveBy(BorderSection section, double deltaMm) =>
        MoveTo(section, section.From + deltaMm);

    /// <summary>
    /// Where a section being moved should start, snapping whichever of its two ends
    /// comes within reach of a target — a moved section keeps its length, so only one
    /// of them can be honoured.
    /// <para>
    /// Only real candidates are scored. The obvious way to write this — snap each end
    /// on its own and keep whichever moved least — silently never snaps the start:
    /// an end that finds no target comes back unchanged, so it scores a distance of
    /// zero, which beats any genuine snap. Only the far end could ever catch, at any
    /// speed, which is what made the whole thing look erratic.
    /// </para>
    /// <para>
    /// <paramref name="wantedFrom"/> is the UNSNAPPED position: feeding the applied
    /// one back in would let the section drift away from the cursor, one correction
    /// at a time.
    /// </para>
    /// </summary>
    public double SnapMovedStart(double wantedFrom, double lengthMm, BorderSection? excluding = null) =>
        BorderSnapEngine.SnapMovedStart(
            wantedFrom, lengthMm, ToMm(SnapToleranceUiPixels), SnapTargetsMm(excluding));

    /// <summary>Slide a section so it starts at <paramref name="fromMm"/>.</summary>
    public void MoveTo(BorderSection section, double fromMm)
    {
        if (BorderSectionGeometry.Move(
                SectionSpans(section),
                new BorderSpan(section.From, section.To),
                fromMm,
                LengthMm) is not { } span)
            return;

        section.From = span.From;
        section.To = span.To;
        // No refresh here: every view of this section follows the model itself.
    }

    public void Delete(BorderSection section) => Side.Sections.Remove(section);

    /// <summary>
    /// Copy a section onto the facing edge of the monitor across it, at the same
    /// absolute position. A passage has to be drawn on both sides to be symmetric —
    /// crossings are per source edge — and forgetting the other half is the single
    /// easiest way to end up with a border that opens one way only by accident.
    /// </summary>
    public bool MirrorToFacingEdge(BorderSection section)
    {
        if (PlanMirror(section) is not { } plan) return false;

        plan.Side.Sections.Add(new BorderSection
        {
            From = plan.From,
            To = plan.To,
            Move = section.Move,
            MoveBlock = section.MoveBlock,
            Drag = section.Drag,
            DragBlock = section.DragBlock
        });

        return true;
    }

    /// <summary>Whether <see cref="MirrorToFacingEdge"/> would do anything.</summary>
    public bool CanMirror(BorderSection section) => PlanMirror(section) != null;

    /// <summary>
    /// Where the copy would land, or null if it cannot: no monitor across this edge,
    /// the facing edge already full, or nothing but a sliver left free there.
    /// <para>
    /// Shared by the action and by the button's enabled state, so the button cannot
    /// promise something the action then refuses. Deliberately free of side effects
    /// and of view models — it is evaluated again on every layout change.
    /// </para>
    /// </summary>
    (BorderSide Side, double From, double To)? PlanMirror(BorderSection section)
    {
        if (FindFacingEdge() is not { } facing) return null;

        var side = SideOf(facing.Monitor, facing.Kind);
        if (side.Sections.Count >= MaximumSections) return null;

        var occupied = side.Sections.Items.Select(s => new BorderSpan(s.From, s.To));
        if (BorderSectionGeometry.PlanMirror(
                OriginMm,
                new BorderSpan(section.From, section.To),
                OriginOf(facing.Monitor, facing.Kind),
                LengthOf(facing.Monitor, facing.Kind),
                occupied,
                MinimumLengthMm) is not { } span)
            return null;

        return (side, span.From, span.To);
    }

    /// <summary>
    /// The monitor and edge across this one, if any.
    /// <para>
    /// The nearest candidate in the right direction that overlaps along the edge and
    /// sits within the layout's travel distance — the same question the link compiler
    /// answers when it decides where a crossing leads, so the mirror agrees with what
    /// the daemon will do.
    /// </para>
    /// <para>
    /// Adjacency cannot be tested by "do the edges touch": these are the VISIBLE
    /// rectangles, and two monitors placed frame against frame have their visible
    /// areas separated by the sum of their two bezels — tens of millimetres. That
    /// mistake made this find a facing edge only on monitors declared without
    /// borders, which is to say almost never.
    /// </para>
    /// </summary>
    public (PhysicalMonitor Monitor, BorderSideKind Kind)? FindFacingEdge()
    {
        var facing = FacingEdgeResolver.FindNearest(
            Kind,
            BoundsOf(Monitor),
            Monitor.Layout.Options.MaxTravelDistance,
            FacingCandidates());

        return facing is { } edge ? (edge.Value, edge.Kind) : null;
    }

    /// <summary>
    /// Every edge across this one, nearest or not. More than one monitor can face a
    /// single edge — two screens stacked against a tall one — and each contributes
    /// its own sections as snap targets, so only the mirror needs to single one out.
    /// </summary>
    public IEnumerable<(PhysicalMonitor Monitor, BorderSideKind Kind)> FacingEdges()
    {
        var facingKind = Kind switch
        {
            BorderSideKind.Left => BorderSideKind.Right,
            BorderSideKind.Right => BorderSideKind.Left,
            BorderSideKind.Top => BorderSideKind.Bottom,
            _ => BorderSideKind.Top
        };

        foreach (var candidate in FacingEdgeResolver.FindAll(
                     Kind,
                     BoundsOf(Monitor),
                     Monitor.Layout.Options.MaxTravelDistance,
                     FacingCandidates()))
            yield return (candidate.Value, facingKind);
    }

    IEnumerable<FacingEdgeCandidate<PhysicalMonitor>> FacingCandidates() =>
        Monitor.Layout.PhysicalMonitors
            .Where(other => !ReferenceEquals(other, Monitor))
            .Select(other => new FacingEdgeCandidate<PhysicalMonitor>(other, BoundsOf(other)));

    public static BorderSide SideOf(PhysicalMonitor monitor, BorderSideKind kind) => kind switch
    {
        BorderSideKind.Left => monitor.BorderResistance.Left,
        BorderSideKind.Top => monitor.BorderResistance.Top,
        BorderSideKind.Right => monitor.BorderResistance.Right,
        _ => monitor.BorderResistance.Bottom
    };
}
