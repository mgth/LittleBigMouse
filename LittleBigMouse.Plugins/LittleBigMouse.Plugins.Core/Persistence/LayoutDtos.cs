#nullable enable
using System.Collections.Generic;
using System.Text.Json.Serialization;

namespace LittleBigMouse.Plugins.Persistence;

// Storage documents shared by every platform backend (registry on Windows, JSON files on
// Linux). Every field is nullable: null means "absent from the store", and the engine
// (LayoutPersistence) then keeps the live model value. Property names are load-bearing on
// both platforms — they ARE the registry value names and the JSON property names — so
// renaming one is a data migration, not a refactoring.
//
// Adding a persisted field = one property here + its Apply/ToDto lines in
// LayoutDtoMapper. No per-OS code.

/// <summary>App-level options, stored once (registry root key / options.json).</summary>
public class GlobalOptionsDto
{
    public string? Priority { get; set; }
    public string? PriorityUnhooked { get; set; }
    public bool? HomeCinema { get; set; }
    public bool? VcpControl { get; set; }
    public bool? Pinned { get; set; }
    public bool? AutoUpdate { get; set; }
    public bool? StartMinimized { get; set; }
    public bool? StartElevated { get; set; }
    public bool? DebugTools { get; set; }
    public bool? ExperimentalFeatures { get; set; }
    public bool? ShowMonitorActionWarning { get; set; }
    public string? BorderValues { get; set; }
    public string? RescueShortcut { get; set; }
    public bool? HideTrayIcon { get; set; }

    /// <summary>
    /// Version of the excluded-defaults top-up already applied (see
    /// <c>ExcludedListPersistence.MigrateDefaults</c>). Not mapped to the options model.
    /// </summary>
    public int? ExcludedDefaultsVersion { get; set; }
}

/// <summary>One layout document (registry Layouts\{id} key / layouts/{id}.json).</summary>
public class LayoutDto
{
    public LayoutOptionsDto? Options { get; set; }
    public Dictionary<string, MonitorDto> Monitors { get; set; } = [];
}

/// <summary>Per-layout options.</summary>
public class LayoutOptionsDto
{
    public bool? AllowOverlaps { get; set; }
    public bool? AllowDiscontinuity { get; set; }
    public string? Algorithm { get; set; }
    public double? MinimalEdgeOverlap { get; set; }
    public double? MaxTravelDistance { get; set; }
    public double? FreelookCheckInterval { get; set; }
    public bool? FreelookEnabled { get; set; }
    public bool? LoopX { get; set; }
    public bool? LoopY { get; set; }
    public bool? Enabled { get; set; }
    public bool? AdjustPointer { get; set; }
    public bool? AdjustSpeed { get; set; }

    /// <summary>
    /// READ-ONLY legacy pair. These are app-level options (see <see cref="GlobalOptionsDto"/>)
    /// that older versions also stored per layout; both locations load into the same options
    /// property, and only the app-level one is written from now on. Kept so a layout that
    /// still carries them keeps its priority on upgrade — after which it is stored once, at
    /// the app level, and this reads null forever.
    /// </summary>
    public string? Priority { get; set; }

    /// <inheritdoc cref="Priority"/>
    public string? PriorityUnhooked { get; set; }
}

public class MonitorDto
{
    public double? XLocationInMm { get; set; }
    public double? YLocationInMm { get; set; }
    public double? PhysicalRatioX { get; set; }
    public double? PhysicalRatioY { get; set; }
    public BorderResistanceDto? BorderResistance { get; set; }

    /// <summary>
    /// Per-monitor bezel borders. PRESENCE is the flag: non-null means the monitor owns its
    /// borders (BordersCustomized); an uncustomized monitor mirrors its model live and
    /// stores nothing here.
    /// </summary>
    public BordersDto? Borders { get; set; }

    public string? ActiveSource { get; set; }

    /// <summary>
    /// WRITE-ONLY: saved from the live model, never applied back. The running value comes
    /// from the EDID at every start, so applying a stored one could only put a stale
    /// serial on a monitor that has since been replaced. It is kept in the store because
    /// it is what makes a dumped configuration readable in a bug report — it says which
    /// physical panel was behind which id.
    /// </summary>
    public string? SerialNumber { get; set; }

    public bool? ExcludedFromLayout { get; set; }
    public Dictionary<string, SourceDto>? Sources { get; set; }
}

/// <summary>
/// A display source's pixel geometry, stored to be restored when the monitor is
/// re-attached to the desktop (only applied on load while the source is detached).
/// </summary>
public class SourceDto
{
    public double? PixelX { get; set; }
    public double? PixelY { get; set; }
    public double? PixelWidth { get; set; }
    public double? PixelHeight { get; set; }
    public int? Orientation { get; set; }

    /// <summary>
    /// WRITE-ONLY, like <see cref="MonitorDto.SerialNumber"/>: both come from the OS at
    /// every start and are saved for what they say about a stored configuration, not to
    /// be restored. Which display is primary is the desktop's business, not ours —
    /// applying a stored value would be LBM deciding it, and a stored device name would
    /// point at whatever slot the OS happened to use last time.
    /// </summary>
    public string? DisplayName { get; set; }

    /// <inheritdoc cref="DisplayName"/>
    public bool? Primary { get; set; }
}

public class BordersDto
{
    public double? Left { get; set; }
    public double? Top { get; set; }
    public double? Right { get; set; }
    public double? Bottom { get; set; }
}

/// <summary>Border resistance, one entry per edge.</summary>
public class BorderResistanceDto
{
    public BorderSideDto? Left { get; set; }
    public BorderSideDto? Top { get; set; }
    public BorderSideDto? Right { get; set; }
    public BorderSideDto? Bottom { get; set; }
}

/// <summary>
/// One edge's resistances. Layouts saved before the move/drag split stored a bare
/// number here instead of an object; <see cref="BorderSideDtoJsonConverter"/> reads
/// both shapes, so the historical key keeps working and nobody's layout resets.
/// </summary>
[JsonConverter(typeof(BorderSideDtoJsonConverter))]
public class BorderSideDto
{
    public double? Move { get; set; }
    public bool? MoveBlock { get; set; }
    public double? Drag { get; set; }
    public bool? DragBlock { get; set; }
    public List<BorderSectionDto>? Sections { get; set; }
}

public class BorderSectionDto
{
    public double? From { get; set; }
    public double? To { get; set; }
    public double? Move { get; set; }
    public bool? MoveBlock { get; set; }
    public double? Drag { get; set; }
    public bool? DragBlock { get; set; }
}

/// <summary>Per-model (PnP code) physical size, shared across layouts.</summary>
public class ModelDto
{
    public double? Width { get; set; }
    public double? Height { get; set; }
    public BordersDto? Borders { get; set; }
    public string? PnpName { get; set; }
}
