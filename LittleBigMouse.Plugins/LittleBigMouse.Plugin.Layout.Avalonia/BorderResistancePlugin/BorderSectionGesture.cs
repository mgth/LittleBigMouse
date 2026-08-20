using LittleBigMouse.DisplayLayout.Dimensions;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>A boundary that landed exactly on a snap target, and what it caught.</summary>
public readonly record struct BorderGuideMark(double Mm, SnapKind Kind);

/// <summary>
/// What a gesture asks its band to draw after a pointer position.
/// <para>
/// Both members are answers, not candidates: which of a moved section's two ends
/// won the guide is decided here, where the targets are known, rather than by the
/// band that draws it.
/// </para>
/// </summary>
/// <param name="Preview">The section being swept out, while one is.</param>
/// <param name="Guide">The boundary to mark, when one landed on a target.</param>
public readonly record struct BorderGestureFeedback(
    BorderSpan? Preview = null,
    BorderGuideMark? Guide = null);

/// <summary>
/// One pointer gesture on a band, from press to release — or to the cancellation
/// that takes its place.
/// <para>
/// Kept apart from the band itself so it can be exercised without a display. That
/// is not a preference: both defects found while testing this feature lived in
/// exactly this logic and in no view — the section that won a boundary two of them
/// shared, and the boundary that jumped under the pointer on press — while the
/// arithmetic they called was covered all along.
/// </para>
/// <para>
/// Nothing here touches Avalonia. The band converts pointer positions into
/// millimetres along its edge and turns the feedback into shapes; every decision
/// about what a press means, and every edit it performs, is here.
/// </para>
/// </summary>
public class BorderSectionGesture(BorderSideViewModel side)
{
    public enum Kind { None, Draw, ResizeFrom, ResizeTo, Move }

    public Kind Mode { get; private set; } = Kind.None;

    /// <summary>The section being resized or moved; null while drawing a new one.</summary>
    public BorderSection? Target { get; private set; }

    public bool Active => Mode != Kind.None;

    /// <summary>Where a drawn section starts, snapped once at the press.</summary>
    double _anchorMm;

    /// <summary>Last raw pointer position, for the moved section's delta.</summary>
    double _lastMm;

    /// <summary>Where a moved section would start with no snapping — see <see cref="Move"/>.</summary>
    double _movedFromMm;

    /// <summary>
    /// Where the target sat when it was taken hold of, to put it back on
    /// <see cref="Cancel"/>.
    /// </summary>
    BorderSpan _grabbedSpan;

    /// <summary>
    /// How far the press landed from the boundary it grabbed.
    /// <para>
    /// A handle is a target several millimetres deep, so a press almost never lands
    /// exactly on the line. Without this the first pointer position moved the
    /// boundary under the pointer, and since one arrives with the press itself the
    /// boundary jumped by the width of the grip before the mouse had moved at all.
    /// </para>
    /// </summary>
    double _grabOffsetMm;

    /// <summary>
    /// Take hold of whatever is under the pointer.
    /// </summary>
    /// <returns>The section to select, or null when the press starts a new one.</returns>
    public BorderSection? Press(double mm, bool snap)
    {
        var (target, grab) = side.GrabAt(mm);

        Target = target;
        Mode = grab switch
        {
            BorderGrab.ResizeFrom => Kind.ResizeFrom,
            BorderGrab.ResizeTo => Kind.ResizeTo,
            BorderGrab.Move => Kind.Move,
            _ => Kind.Draw
        };

        // Held from wherever inside the handle the press landed, so the boundary
        // follows the pointer's movement rather than its position.
        _grabOffsetMm = Mode switch
        {
            Kind.ResizeFrom when target != null => mm - target.From,
            Kind.ResizeTo when target != null => mm - target.To,
            _ => 0
        };

        _anchorMm = side.Snap(mm, snap, target);
        _lastMm = mm;
        _movedFromMm = target?.From ?? 0;
        _grabbedSpan = target == null ? default : new BorderSpan(target.From, target.To);

        return target;
    }

    /// <summary>Apply a new pointer position, and say what should be drawn.</summary>
    public BorderGestureFeedback Move(double raw, bool snap)
    {
        // The grip is held where it was taken; the offset is zero for the gestures
        // that do not grab a boundary.
        var mm = side.Snap(raw - _grabOffsetMm, snap, Target);

        switch (Mode)
        {
            case Kind.Draw:
                // A section is swept out from a fixed anchor, so only the moving end
                // can catch anything.
                return new BorderGestureFeedback(new BorderSpan(_anchorMm, mm), Mark(snap, mm));

            case Kind.ResizeFrom when Target != null:
                side.Resize(Target, mm, Target.To);
                return new BorderGestureFeedback(Guide: Mark(snap, mm));

            case Kind.ResizeTo when Target != null:
                side.Resize(Target, Target.From, mm);
                return new BorderGestureFeedback(Guide: Mark(snap, mm));

            case Kind.Move when Target != null:
                // The wanted position follows the pointer exactly and is kept apart
                // from the applied one: feeding the snapped position back into the
                // next delta would let the section drift away from the cursor, one
                // correction at a time.
                _movedFromMm += raw - _lastMm;
                _lastMm = raw;

                var length = Target.To - Target.From;
                side.MoveTo(Target, snap
                    ? side.SnapMovedStart(_movedFromMm, length, Target)
                    : _movedFromMm);

                // A moved section keeps its length, so at most one of its ends can
                // have caught a target; the far one is tried when the near one found
                // nothing.
                return new BorderGestureFeedback(Guide: Mark(snap, Target.From, Target.To));

            default:
                return new BorderGestureFeedback();
        }
    }

    /// <summary>
    /// The first of these boundaries that landed on a snap target, if any. The
    /// section being dragged is left out, or it would keep catching itself.
    /// </summary>
    BorderGuideMark? Mark(bool snap, params double[] candidates)
    {
        if (!snap) return null;

        foreach (var mm in candidates)
        {
            if (side.MatchedTarget(mm, Target) is { } target) return new BorderGuideMark(mm, target.Kind);
        }

        return null;
    }

    /// <summary>
    /// Let go. A gesture that was drawing creates its section here, since only its
    /// release says how far it swept.
    /// </summary>
    /// <returns>The section created, if any.</returns>
    public BorderSection? Release(double raw, bool snap)
    {
        BorderSection? created = null;

        if (Mode == Kind.Draw)
        {
            created = side.Create(_anchorMm, side.Snap(raw, snap, Target));
        }

        Idle();

        return created;
    }

    /// <summary>
    /// Give up on the gesture and leave the edge as it was found.
    /// <para>
    /// A drawn section only exists once released, so there is nothing to undo there;
    /// a resize or a move has been applied at every pointer position since the press,
    /// and is put back where it was taken from. Directly, not through
    /// <see cref="BorderSideViewModel.Resize"/>: the span being restored held that
    /// place a moment ago, so there is nothing left to clamp it against.
    /// </para>
    /// <para>
    /// This is where a lost capture ends up too. Losing the pointer mid-drag is not a
    /// decision to keep whatever the section happened to be under at that instant.
    /// </para>
    /// </summary>
    /// <returns>The section put back, if the gesture had edited one.</returns>
    public BorderSection? Cancel()
    {
        var restored = Mode is Kind.ResizeFrom or Kind.ResizeTo or Kind.Move ? Target : null;

        if (restored != null)
        {
            restored.From = _grabbedSpan.From;
            restored.To = _grabbedSpan.To;
        }

        Idle();

        return restored;
    }

    void Idle()
    {
        Mode = Kind.None;
        Target = null;
        _grabOffsetMm = 0;
        _grabbedSpan = default;
    }
}
