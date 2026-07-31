using System.Text.Json.Serialization;

namespace LittleBigMouse.Zoning;

/// <summary>
/// The resistances governing one stretch of edge, as resolved by the link
/// compiler: either a section's or the side's default.
/// </summary>
public readonly record struct BorderResistanceValues(
    double Move,
    bool MoveBlock,
    double Drag,
    bool DragBlock);

public class ZoneLink : IZonesSerializable
{
    public double Distance { get; set; }
    public double From { get; set; }
    public double To { get; set; }
    public int SourceFromPixel { get; set; }
    public int SourceToPixel { get; set; }
    public int TargetFromPixel { get; set; }
    public int TargetToPixel { get; set; }

    /// <summary>
    /// Resistance opposing a plain move. Keeps its historical name on the wire:
    /// a daemon that predates the move/drag split reads it and behaves as before.
    /// </summary>
    public double BorderResistance { get; set; }

    /// <summary>When set, no plain move crosses this stretch at all.</summary>
    public bool MoveBlock { get; set; }

    /// <summary>
    /// Resistance opposing a move made with a mouse button held. A daemon that
    /// doesn't know this attribute falls back to <see cref="BorderResistance"/>.
    /// </summary>
    public double DragResistance { get; set; }

    /// <summary>When set, no drag crosses this stretch at all.</summary>
    public bool DragBlock { get; set; }

    [JsonIgnore] public Zone? Target { get; set; }
    public int TargetId => Target?.Id??-1;

    /// <summary>
    /// Whether this link is indistinguishable from <paramref name="other"/> to the
    /// daemon, and may therefore be merged with it.
    /// </summary>
    public bool HasSameResistanceAs(BorderResistanceValues other) =>
        BorderResistance.Equals(other.Move)
        && MoveBlock == other.MoveBlock
        && DragResistance.Equals(other.Drag)
        && DragBlock == other.DragBlock;

    public string Serialize()
    {
        return ZoneSerializer.Serialize(this,
            e => e.From,
            e => e.To,
            e => e.SourceFromPixel,
            e => e.SourceToPixel,
            e => e.TargetFromPixel,
            e => e.TargetToPixel,
            e => e.BorderResistance,
            e => e.MoveBlock,
            e => e.DragResistance,
            e => e.DragBlock,

            e => e.TargetId );
    }
}
