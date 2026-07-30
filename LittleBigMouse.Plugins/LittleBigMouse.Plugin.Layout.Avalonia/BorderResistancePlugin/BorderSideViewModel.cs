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

public enum BorderSideKind { Left, Top, Right, Bottom }

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

        _subscriptions.Add(this.WhenAnyValue(e => e.PixelLength)
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
    /// </summary>
    public IReadOnlyList<double> SnapTargetsMm()
    {
        var targets = new List<double> { 0, LengthMm, LengthMm / 2 };

        foreach (var other in Monitor.Layout.PhysicalMonitors)
        {
            if (ReferenceEquals(other, Monitor)) continue;

            var bounds = other.DepthProjection.Bounds;
            var (near, far) = IsVertical
                ? (bounds.Top, bounds.Bottom)
                : (bounds.Left, bounds.Right);

            AddIfOnEdge(near - OriginMm);
            AddIfOnEdge(far - OriginMm);
        }

        return targets;

        void AddIfOnEdge(double value)
        {
            if (value < 0 || value > LengthMm) return;
            if (targets.Any(t => System.Math.Abs(t - value) < 0.001)) return;
            targets.Add(value);
        }
    }

    /// <summary>Snap <paramref name="mm"/> to the nearest target within tolerance.</summary>
    public double Snap(double mm, bool enabled = true)
    {
        var clamped = System.Math.Clamp(mm, 0, LengthMm);
        if (!enabled) return clamped;

        var tolerance = ToMm(SnapToleranceUiPixels);
        var best = clamped;
        var bestDistance = tolerance;

        foreach (var target in SnapTargetsMm())
        {
            var distance = System.Math.Abs(target - clamped);
            if (distance >= bestDistance) continue;
            bestDistance = distance;
            best = target;
        }

        return best;
    }

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
    public (double Low, double High) FreeGapAround(double referenceMm, BorderSection? excluding) =>
        FreeGapAround(Side.Sections.Items, referenceMm, LengthMm, excluding);

    /// <summary>
    /// The longest stretch of <c>[from, to]</c> that no section occupies, or null if
    /// the range is entirely taken. Used by the mirror, which has to settle for the
    /// part of the facing edge that is actually free rather than give up on the whole
    /// copy because one end is occupied.
    /// </summary>
    static (double From, double To)? LargestFreeSpan(
        IEnumerable<BorderSection> sections, double from, double to)
    {
        var best = (From: 0.0, To: 0.0);
        var cursor = from;

        foreach (var section in sections
                     .Where(s => s.To > from && s.From < to)
                     .OrderBy(s => s.From))
        {
            if (section.From > cursor) Consider(cursor, System.Math.Min(section.From, to));

            cursor = System.Math.Max(cursor, section.To);
            if (cursor >= to) break;
        }

        if (cursor < to) Consider(cursor, to);

        return best.To > best.From ? best : null;

        void Consider(double low, double high)
        {
            if (high - low > best.To - best.From) best = (low, high);
        }
    }

    /// <summary>Same, on any edge's sections — the mirror measures the facing one.</summary>
    static (double Low, double High) FreeGapAround(
        IEnumerable<BorderSection> sections, double referenceMm, double lengthMm, BorderSection? excluding)
    {
        var low = 0.0;
        var high = lengthMm;

        foreach (var other in sections.OrderBy(s => s.From))
        {
            if (ReferenceEquals(other, excluding)) continue;

            if (other.From < referenceMm && other.To > referenceMm) return (referenceMm, referenceMm);

            if (other.To <= referenceMm && other.To > low) low = other.To;
            if (other.From >= referenceMm && other.From < high) high = other.From;
        }

        return (low, high);
    }

    /// <summary>Restrict [from, to] to the free gap containing <paramref name="referenceMm"/>.</summary>
    public (double From, double To) ClampToFreeSpace(double from, double to, double referenceMm, BorderSection? excluding)
    {
        var (low, high) = FreeGapAround(referenceMm, excluding);
        return (System.Math.Max(from, low), System.Math.Min(to, high));
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

        var (from, to) = Order(anchorMm, toMm);
        (from, to) = ClampToFreeSpace(from, to, anchorMm, null);
        if (to - from < MinimumLengthMm) return null;

        var section = new BorderSection
        {
            From = from,
            To = to,
            // A fresh section starts as a wall for plain moves and free for drags:
            // that is the #458 shape (a barrier you can still drag a window across),
            // and it makes the section visible the moment it is drawn.
            MoveBlock = true
        };

        Side.Sections.Add(section);
        return section;
    }

    public void Resize(BorderSection section, double fromMm, double toMm)
    {
        // The section's own midpoint identifies the gap it lives in, whichever end
        // is being dragged.
        var reference = (section.From + section.To) / 2;

        var (from, to) = Order(fromMm, toMm);
        (from, to) = ClampToFreeSpace(from, to, reference, section);
        if (to - from < MinimumLengthMm) return;

        section.From = from;
        section.To = to;
        // No refresh here: every view of this section follows the model itself.
    }

    /// <summary>
    /// Slide a section along its edge, keeping its length. Pushed into a neighbour it
    /// comes to rest against it rather than refusing to move.
    /// </summary>
    public void MoveBy(BorderSection section, double deltaMm)
    {
        var length = section.To - section.From;
        var (low, high) = FreeGapAround((section.From + section.To) / 2, section);
        if (high - low < length) return;

        var from = System.Math.Clamp(section.From + deltaMm, low, high - length);

        section.From = from;
        section.To = from + length;
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

        var origin = OriginOf(facing.Monitor, facing.Kind);
        var length = LengthOf(facing.Monitor, facing.Kind);

        // Same absolute layout coordinates, re-expressed against the other edge.
        var from = System.Math.Clamp(OriginMm + section.From - origin, 0, length);
        var to = System.Math.Clamp(OriginMm + section.To - origin, 0, length);

        // The longest free stretch INSIDE the wanted range, not the gap around its
        // midpoint: the facing edge is often partly taken, and the two ranges then
        // overlap only at one end. Probing the midpoint declared the whole mirror
        // impossible whenever that midpoint happened to fall on an existing section,
        // even with most of the range free.
        if (LargestFreeSpan(side.Sections.Items, from, to) is not { } span) return null;
        if (span.To - span.From < MinimumLengthMm) return null;

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
        var mine = Monitor.DepthProjection.Bounds;

        PhysicalMonitor? best = null;
        var bestGap = Monitor.Layout.Options.MaxTravelDistance;

        foreach (var other in Monitor.Layout.PhysicalMonitors)
        {
            if (ReferenceEquals(other, Monitor)) continue;

            var bounds = other.DepthProjection.Bounds;

            var gap = Kind switch
            {
                BorderSideKind.Left => mine.Left - bounds.Right,
                BorderSideKind.Right => bounds.Left - mine.Right,
                BorderSideKind.Top => mine.Top - bounds.Bottom,
                _ => bounds.Top - mine.Bottom
            };

            // Behind us, or further than the cursor would ever travel.
            if (gap < 0 || gap > bestGap) continue;

            var overlaps = IsVertical
                ? bounds.Bottom > mine.Top && bounds.Top < mine.Bottom
                : bounds.Right > mine.Left && bounds.Left < mine.Right;

            if (!overlaps) continue;

            best = other;
            bestGap = gap;
        }

        if (best == null) return null;

        return (best, Kind switch
        {
            BorderSideKind.Left => BorderSideKind.Right,
            BorderSideKind.Right => BorderSideKind.Left,
            BorderSideKind.Top => BorderSideKind.Bottom,
            _ => BorderSideKind.Top
        });
    }

    public static BorderSide SideOf(PhysicalMonitor monitor, BorderSideKind kind) => kind switch
    {
        BorderSideKind.Left => monitor.BorderResistance.Left,
        BorderSideKind.Top => monitor.BorderResistance.Top,
        BorderSideKind.Right => monitor.BorderResistance.Right,
        _ => monitor.BorderResistance.Bottom
    };

    static (double From, double To) Order(double a, double b) => a <= b ? (a, b) : (b, a);
}
