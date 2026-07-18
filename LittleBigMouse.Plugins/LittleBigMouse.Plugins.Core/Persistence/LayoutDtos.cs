#nullable enable
using System.Collections.Generic;

namespace LittleBigMouse.Plugins.Persistence;

// Storage documents shared by every platform backend (registry on Windows, JSON files on
// Linux). Every field is nullable: null means "absent from the store", and the engine
// (LayoutPersistence) then keeps the live model value. Property names are load-bearing on
// both platforms — they ARE the registry value names and the JSON property names — so
// renaming one is a data migration, not a refactoring.
//
// Adding a persisted field = one property here + its Apply/ToDto lines in
// LayoutPersistence. No per-OS code.

/// <summary>App-level options, stored once (registry root key / options.json).</summary>
public class GlobalOptionsDto
{
    public int? DaemonPort { get; set; }
    public string? Priority { get; set; }
    public string? PriorityUnhooked { get; set; }
    public bool? HomeCinema { get; set; }
    public bool? Pinned { get; set; }
    public bool? AutoUpdate { get; set; }
    public bool? StartMinimized { get; set; }
    public bool? StartElevated { get; set; }
    public bool? DebugTools { get; set; }
    public bool? ShowMonitorActionWarning { get; set; }
    public string? BorderValues { get; set; }
    public bool? HideTrayIcon { get; set; }

    /// <summary>
    /// Version of the excluded-defaults top-up already applied (see
    /// <c>LayoutPersistence.MigrateExcludedDefaults</c>). Not mapped to the options model.
    /// </summary>
    public int? ExcludedDefaultsVersion { get; set; }
}

/// <summary>One layout document (registry Layouts\{id} key / layouts/{id}.json).</summary>
public class LayoutDto
{
    public LayoutOptionsDto? Options { get; set; }
    public Dictionary<string, MonitorDto> Monitors { get; set; } = [];
}

/// <summary>Per-layout options. Priority/PriorityUnhooked here override the global ones.</summary>
public class LayoutOptionsDto
{
    public bool? AllowOverlaps { get; set; }
    public bool? AllowDiscontinuity { get; set; }
    public string? Algorithm { get; set; }
    public double? MaxTravelDistance { get; set; }
    public double? FreelookCheckInterval { get; set; }
    public bool? FreelookEnabled { get; set; }
    public bool? LoopX { get; set; }
    public bool? LoopY { get; set; }
    public bool? Enabled { get; set; }
    public bool? AdjustPointer { get; set; }
    public bool? AdjustSpeed { get; set; }
    public string? Priority { get; set; }
    public string? PriorityUnhooked { get; set; }
}

public class MonitorDto
{
    public double? XLocationInMm { get; set; }
    public double? YLocationInMm { get; set; }
    public double? PhysicalRatioX { get; set; }
    public double? PhysicalRatioY { get; set; }
    public BordersDto? BorderResistance { get; set; }

    /// <summary>
    /// Per-monitor bezel borders. PRESENCE is the flag: non-null means the monitor owns its
    /// borders (BordersCustomized); an uncustomized monitor mirrors its model live and
    /// stores nothing here.
    /// </summary>
    public BordersDto? Borders { get; set; }

    public string? ActiveSource { get; set; }
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
    public string? DisplayName { get; set; }
    public bool? Primary { get; set; }
}

public class BordersDto
{
    public double? Left { get; set; }
    public double? Top { get; set; }
    public double? Right { get; set; }
    public double? Bottom { get; set; }
}

/// <summary>Per-model (PnP code) physical size, shared across layouts.</summary>
public class ModelDto
{
    public double? Width { get; set; }
    public double? Height { get; set; }
    public BordersDto? Borders { get; set; }
    public string? PnpName { get; set; }
}
