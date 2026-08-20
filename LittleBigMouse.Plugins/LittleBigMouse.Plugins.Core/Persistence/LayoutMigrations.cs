#nullable enable
using System.Collections.Generic;

namespace LittleBigMouse.Plugins.Persistence;

/// <summary>
/// Every reinterpretation of OLD stored data, gathered so the mapping stays a plain
/// field-for-field copy. A migration lives here when reading a value requires knowing
/// WHEN it was written; the shape-level compatibility (a bare number where an object is
/// now expected) belongs to the stores instead — <see cref="BorderSideDtoJsonConverter"/>
/// on Linux, <c>RegistryLayoutStore.ReadSide</c> on Windows — because it is the
/// deserialization that would otherwise fail.
/// <para>
/// The migrations are independent of one another and are applied per value, in the order
/// the mapper reads the DTO. None of them writes: a migrated value only reaches the store
/// at the next save, when it is written in the current shape, and re-reading it is then a
/// permanent no-op. History (oldest first):
/// </para>
/// <list type="number">
/// <item><see cref="Sections"/> — pre-section-editor whole-edge resistance (one Move/Drag
/// pair per edge) becomes the section that says the same thing.</item>
/// <item><see cref="StoredModelSize"/>, 0x0 case — pre-EDID-fallback placeholder size
/// (#419) must not override the freshly computed one.</item>
/// <item><see cref="NormalizeStoredSize"/> — pre-5.4.1 model sizes were stored ORIENTED
/// to the rotation at save time; they are transposed back to intrinsic (#507).</item>
/// </list>
/// <para>
/// The one-time top-up of the default exclusion list is a migration too, but it is
/// stateful (it reads and writes files and a version counter) and lives with the list it
/// migrates, in <see cref="ExcludedListPersistence"/>.
/// </para>
/// </summary>
public static class LayoutMigrations
{
    /// <summary>
    /// The sections an edge actually holds. Stored sections win when there are any;
    /// otherwise an edge saved before the section editor carried a single resistance over
    /// its whole length, and rather than keep that notion alongside the sections it
    /// becomes the section that says the same thing — so an existing setting stays in
    /// force AND shows up in the editor, where it can be split or trimmed like any other.
    /// <para>
    /// <paramref name="edgeLengthMm"/> is the edge's current length, needed to give the
    /// migrated section its extent. A non-positive length (nothing to span) drops the
    /// legacy value — see the caveat on <see cref="LegacyWholeEdgeSection"/>.
    /// </para>
    /// </summary>
    public static IReadOnlyList<BorderSectionDto> Sections(BorderSideDto dto, double edgeLengthMm)
    {
        if (dto.Sections is { Count: > 0 } stored) return stored;

        var legacy = LegacyWholeEdgeSection(dto, edgeLengthMm);
        return legacy == null ? [] : [legacy];
    }

    /// <summary>
    /// The whole-edge section standing for a pre-section-editor resistance, or null when
    /// there is nothing to migrate.
    /// <para>
    /// CAVEAT: a legacy resistance on an edge of unknown length is dropped rather than
    /// migrated — the section needs an extent and there is none to give it. In practice
    /// the length comes from the monitor's depth projection, which is known by the time
    /// the layout loads; this only bites a monitor whose size could not be computed at
    /// all, which has no working resistance either way.
    /// </para>
    /// </summary>
    public static BorderSectionDto? LegacyWholeEdgeSection(BorderSideDto dto, double edgeLengthMm)
    {
        if (edgeLengthMm <= 0) return null;

        var move = dto.Move ?? 0;
        var drag = dto.Drag ?? 0;
        var moveBlock = dto.MoveBlock ?? false;
        var dragBlock = dto.DragBlock ?? false;

        // A zero, unblocked edge is what "no resistance" has always looked like:
        // migrating it would litter every layout with meaningless full-edge sections.
        if (move <= 0 && drag <= 0 && !moveBlock && !dragBlock) return null;

        return new BorderSectionDto
        {
            From = 0,
            To = edgeLengthMm,
            Move = move,
            MoveBlock = moveBlock,
            Drag = drag,
            DragBlock = dragBlock
        };
    }

    /// <summary>
    /// The stored physical size to apply over the freshly computed (intrinsic) one. Null
    /// components mean "keep the computed value".
    /// <para>
    /// Versions predating the EDID-less size fallback persisted the bogus 0x0 GDI
    /// placeholder for virtual displays (#419): a stored non-positive size must not
    /// override the computed one. A complete stored size then goes through
    /// <see cref="NormalizeStoredSize"/>; a half-valid one (one dimension only) is applied
    /// as-is, since a single dimension says nothing about orientation.
    /// </para>
    /// </summary>
    public static (double? Width, double? Height) StoredModelSize(
        double intrinsicWidth, double intrinsicHeight,
        double? storedWidth, double? storedHeight)
    {
        if (storedWidth is > 0 && storedHeight is > 0)
        {
            var (w, h) = NormalizeStoredSize(
                intrinsicWidth, intrinsicHeight, storedWidth.Value, storedHeight.Value);
            return (w, h);
        }

        return (storedWidth is > 0 ? storedWidth : null, storedHeight is > 0 ? storedHeight : null);
    }

    /// <summary>
    /// Migration of pre-5.4.1 stored model sizes (#507). The model used to persist the
    /// size ORIENTED to the display's rotation at save time; since 5.4.1 it stores the
    /// intrinsic panel size and the projection chain applies the rotation downstream. A
    /// stored portrait-oriented size read as intrinsic gets the rotation applied twice:
    /// the monitor that was portrait at save time renders with the orientation inverted
    /// after the upgrade — flipping the display in Windows shows exactly the opposite in
    /// LBM. The freshly computed model size is intrinsic by construction: when the stored
    /// orientation contradicts it, transpose the stored value — the portrait/landscape
    /// signal is robust to user-customized magnitudes (edits keep the panel aspect via
    /// FixedAspectRatio), which are preserved. Square or invalid sizes decide nothing.
    /// Once the layout is saved again the store holds the intrinsic size and this is a
    /// permanent no-op.
    /// </summary>
    public static (double Width, double Height) NormalizeStoredSize(
        double intrinsicWidth, double intrinsicHeight,
        double storedWidth, double storedHeight)
    {
        if (intrinsicWidth <= 0 || intrinsicHeight <= 0
            || intrinsicWidth == intrinsicHeight || storedWidth == storedHeight)
            return (storedWidth, storedHeight);

        var intrinsicPortrait = intrinsicHeight > intrinsicWidth;
        var storedPortrait = storedHeight > storedWidth;

        return storedPortrait == intrinsicPortrait
            ? (storedWidth, storedHeight)
            : (storedHeight, storedWidth);
    }
}
