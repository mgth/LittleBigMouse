namespace LittleBigMouse.Zoning;

/// <summary>
/// A user-drawn stretch of one monitor edge, carrying its own resistances.
/// <para>
/// <see cref="From"/> and <see cref="To"/> are millimetres measured from the edge's
/// starting corner — the top for <see cref="IBorderResistance.Left"/> and
/// <see cref="IBorderResistance.Right"/>, the left for <see cref="IBorderResistance.Top"/>
/// and <see cref="IBorderResistance.Bottom"/>. Being relative to the monitor, they
/// survive moving it around the layout.
/// </para>
/// <para>
/// Sections never reach the daemon: <c>Zone.ComputeLinks</c> folds them into the
/// per-side <c>ZoneLink</c> subdivision it already builds, so they cost the hook
/// nothing at runtime.
/// </para>
/// </summary>
public interface IBorderSection
{
    double From { get; }
    double To { get; }

    /// <summary>Resistance in mm opposing a plain cursor move.</summary>
    double Move { get; }

    /// <summary>When set, no plain move crosses here at all.</summary>
    bool MoveBlock { get; }

    /// <summary>Resistance in mm opposing a move made with a mouse button held.</summary>
    double Drag { get; }

    /// <summary>When set, no drag crosses here at all.</summary>
    bool DragBlock { get; }
}

/// <summary>
/// One edge of a monitor: a default resistance pair, plus any number of sections
/// overriding it over part of the edge.
/// </summary>
public interface IBorderSide
{
    /// <summary>Resistance applied wherever no section covers the edge.</summary>
    double Move { get; }

    bool MoveBlock { get; }

    double Drag { get; }

    bool DragBlock { get; }

    IReadOnlyList<IBorderSection> Sections { get; }
}

public interface IBorderResistance
{
    IBorderSide Left { get; }
    IBorderSide Top { get; }
    IBorderSide Right { get; }
    IBorderSide Bottom { get; }
}
