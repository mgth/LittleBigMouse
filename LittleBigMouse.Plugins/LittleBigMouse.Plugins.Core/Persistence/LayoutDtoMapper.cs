#nullable enable
using System.Linq;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins.Persistence;

/// <summary>
/// The whole model↔DTO mapping, once, for every platform. <c>Apply</c> reads a DTO into
/// the live model (null field = absent from the store = keep the live value),
/// <c>ToDto</c> writes the model back. The two directions stay in this one file on
/// purpose: adding a persisted field means one DTO property (LayoutDtos.cs) and its two
/// mapping lines here — no per-OS code, so Windows/Linux can no longer drift apart.
/// <para>
/// Nothing here interprets history: reading a value that means something different
/// depending on when it was written goes through <see cref="LayoutMigrations"/>.
/// </para>
/// </summary>
public static class LayoutDtoMapper
{
    //==================//
    // DTO -> model     //
    //==================//

    public static void Apply(ILayoutOptions o, GlobalOptionsDto? dto)
    {
        if (dto == null) return;

        o.DaemonPort = dto.DaemonPort ?? o.DaemonPort;
        o.Priority = dto.Priority ?? o.Priority;
        o.PriorityUnhooked = dto.PriorityUnhooked ?? o.PriorityUnhooked;
        o.HomeCinema = dto.HomeCinema ?? o.HomeCinema;
        o.Pinned = dto.Pinned ?? o.Pinned;
        o.AutoUpdate = dto.AutoUpdate ?? o.AutoUpdate;
        o.StartMinimized = dto.StartMinimized ?? o.StartMinimized;
        o.StartElevated = dto.StartElevated ?? o.StartElevated;
        o.DebugTools = dto.DebugTools ?? o.DebugTools;
        o.VcpControl = dto.VcpControl ?? o.VcpControl;
        o.ShowMonitorActionWarning = dto.ShowMonitorActionWarning ?? o.ShowMonitorActionWarning;
        o.BorderValues = dto.BorderValues ?? o.BorderValues;
        o.RescueShortcut = dto.RescueShortcut ?? o.RescueShortcut;
        o.HideTrayIcon = dto.HideTrayIcon ?? o.HideTrayIcon;
    }

    public static void Apply(ILayoutOptions o, LayoutOptionsDto? dto)
    {
        if (dto == null) return;

        o.AllowOverlaps = dto.AllowOverlaps ?? o.AllowOverlaps;
        o.AllowDiscontinuity = dto.AllowDiscontinuity ?? o.AllowDiscontinuity;
        o.Algorithm = dto.Algorithm ?? o.Algorithm;
        o.MaxTravelDistance = dto.MaxTravelDistance ?? o.MaxTravelDistance;
        o.FreelookCheckInterval = dto.FreelookCheckInterval ?? o.FreelookCheckInterval;
        o.FreelookEnabled = dto.FreelookEnabled ?? o.FreelookEnabled;
        o.LoopX = dto.LoopX ?? o.LoopX;
        o.LoopY = dto.LoopY ?? o.LoopY;
        o.Enabled = dto.Enabled ?? o.Enabled;
        o.AdjustPointer = dto.AdjustPointer ?? o.AdjustPointer;
        o.AdjustSpeed = dto.AdjustSpeed ?? o.AdjustSpeed;

        // READ-ONLY legacy: the app-level values are applied first, and a layout that
        // still carries its own overrides them so nobody's priority changes on upgrade.
        // Nothing writes them back (see ToDto below), so this ends at the first save.
        o.Priority = dto.Priority ?? o.Priority;
        o.PriorityUnhooked = dto.PriorityUnhooked ?? o.PriorityUnhooked;
    }

    public static void Apply(PhysicalMonitorModel model, ModelDto dto)
    {
        var fixedRatio = model.PhysicalSize.FixedAspectRatio;
        model.PhysicalSize.FixedAspectRatio = false;

        model.PhysicalSize.TopBorder = dto.Borders?.Top ?? model.PhysicalSize.TopBorder;
        model.PhysicalSize.RightBorder = dto.Borders?.Right ?? model.PhysicalSize.RightBorder;
        model.PhysicalSize.BottomBorder = dto.Borders?.Bottom ?? model.PhysicalSize.BottomBorder;
        model.PhysicalSize.LeftBorder = dto.Borders?.Left ?? model.PhysicalSize.LeftBorder;

        // Placeholder sizes (#419) and pre-5.4.1 oriented sizes (#507) — see LayoutMigrations.
        var (width, height) = LayoutMigrations.StoredModelSize(
            model.PhysicalSize.Width, model.PhysicalSize.Height, dto.Width, dto.Height);

        if (width is { } w) model.PhysicalSize.Width = w;
        if (height is { } h) model.PhysicalSize.Height = h;

        model.PhysicalSize.FixedAspectRatio = fixedRatio;

        if (!string.IsNullOrEmpty(dto.PnpName)) model.PnpDeviceName = dto.PnpName;
    }

    public static void Apply(PhysicalMonitor monitor, MonitorDto dto)
    {
        foreach (var source in monitor.Sources.Items)
        {
            if (dto.Sources == null || !dto.Sources.TryGetValue(source.Source.Id, out var s)) continue;

            // Detached sources restore their stored pixel geometry (nothing current to
            // keep); attached ones keep the live geometry, the store is just a backup.
            if (!source.Source.AttachedToDesktop)
            {
                source.Source.InPixel.Set(new HLab.Geo.Rect(
                    new HLab.Geo.Point(s.PixelX ?? source.Source.InPixel.X, s.PixelY ?? source.Source.InPixel.Y),
                    new HLab.Geo.Size(s.PixelWidth ?? source.Source.InPixel.Width, s.PixelHeight ?? source.Source.InPixel.Height)));
                source.Source.Orientation = s.Orientation ?? source.Source.Orientation;
            }

            if (dto.ActiveSource != null && source.Source.Id == dto.ActiveSource)
                monitor.ActiveSource = source;
        }

        if (dto.XLocationInMm is { } x) { monitor.DepthProjection.X = x; monitor.Placed = true; }
        if (dto.YLocationInMm is { } y) { monitor.DepthProjection.Y = y; monitor.Placed = true; }

        monitor.DepthRatio.X = dto.PhysicalRatioX ?? monitor.DepthRatio.X;
        monitor.DepthRatio.Y = dto.PhysicalRatioY ?? monitor.DepthRatio.Y;

        // The edge length is needed to convert a stored whole-edge resistance into
        // the section that now expresses it.
        var acrossMm = monitor.DepthProjection.Width;
        var downMm = monitor.DepthProjection.Height;

        Apply(monitor.BorderResistance.Left, dto.BorderResistance?.Left, downMm);
        Apply(monitor.BorderResistance.Top, dto.BorderResistance?.Top, acrossMm);
        Apply(monitor.BorderResistance.Right, dto.BorderResistance?.Right, downMm);
        Apply(monitor.BorderResistance.Bottom, dto.BorderResistance?.Bottom, acrossMm);

        monitor.ExcludedFromLayout = dto.ExcludedFromLayout ?? monitor.ExcludedFromLayout;

        // Per-monitor bezel borders load whatever the current mode is, so switching to
        // PerMonitor is live (no restart required). Stored values only exist once the
        // user edited them in PerMonitor mode: until then Borders keep mirroring the
        // live model values, so the FIRST switch starts from the monitor's current
        // PerModel borders.
        if (dto.Borders != null)
        {
            monitor.Borders.Left = dto.Borders.Left ?? monitor.Borders.Left;
            monitor.Borders.Top = dto.Borders.Top ?? monitor.Borders.Top;
            monitor.Borders.Right = dto.Borders.Right ?? monitor.Borders.Right;
            monitor.Borders.Bottom = dto.Borders.Bottom ?? monitor.Borders.Bottom;
            monitor.BordersCustomized = true;
        }
    }

    public static void Apply(BorderSide side, BorderSideDto? dto, double edgeLengthMm)
    {
        if (dto == null) return;

        side.Sections.Edit(list =>
        {
            list.Clear();

            foreach (var s in LayoutMigrations.Sections(dto, edgeLengthMm))
            {
                list.Add(new BorderSection
                {
                    From = s.From ?? 0,
                    To = s.To ?? 0,
                    Move = s.Move ?? 0,
                    MoveBlock = s.MoveBlock ?? false,
                    Drag = s.Drag ?? 0,
                    DragBlock = s.DragBlock ?? false
                });
            }
        });
    }

    //==================//
    // model -> DTO     //
    //==================//

    /// <summary>
    /// App-level options. <paramref name="excludedDefaultsVersion"/> is not part of the
    /// options model: it is read at load time and round-tripped through every write so the
    /// one-time exclusion-list top-up stays one-time (see <see cref="ExcludedListPersistence"/>).
    /// </summary>
    public static GlobalOptionsDto ToGlobalOptionsDto(ILayoutOptions o, int? excludedDefaultsVersion) => new()
    {
        DaemonPort = o.DaemonPort,
        Priority = o.Priority,
        PriorityUnhooked = o.PriorityUnhooked,
        HomeCinema = o.HomeCinema,
        Pinned = o.Pinned,
        AutoUpdate = o.AutoUpdate,
        StartMinimized = o.StartMinimized,
        StartElevated = o.StartElevated,
        DebugTools = o.DebugTools,
        VcpControl = o.VcpControl,
        ShowMonitorActionWarning = o.ShowMonitorActionWarning,
        BorderValues = o.BorderValues,
        RescueShortcut = o.RescueShortcut,
        HideTrayIcon = o.HideTrayIcon,
        ExcludedDefaultsVersion = excludedDefaultsVersion
    };

    public static LayoutDto ToLayoutDto(MonitorsLayout layout) => new()
    {
        Options = ToDto(layout.Options),
        Monitors = layout.PhysicalMonitors.ToDictionary(m => m.Id, ToDto)
    };

    // Priority/PriorityUnhooked are deliberately absent: they are app-level options that
    // once lived here too, and both locations load into the SAME options property. Writing
    // the property back to both made a layout's leftover value the app-level one at the
    // next save, and gave a copy of it to every other layout after that. They are written
    // to the app level only; the layout copy is read as a fallback and then dropped.
    public static LayoutOptionsDto ToDto(ILayoutOptions o) => new()
    {
        AllowOverlaps = o.AllowOverlaps,
        AllowDiscontinuity = o.AllowDiscontinuity,
        Algorithm = o.Algorithm,
        MaxTravelDistance = o.MaxTravelDistance,
        FreelookCheckInterval = o.FreelookCheckInterval,
        FreelookEnabled = o.FreelookEnabled,
        LoopX = o.LoopX,
        LoopY = o.LoopY,
        Enabled = o.Enabled,
        AdjustPointer = o.AdjustPointer,
        AdjustSpeed = o.AdjustSpeed
    };

    // Sections only: the per-edge resistance is gone, and writing it back would
    // resurrect it on the next load through the migration path. Always emitted, even
    // empty, so deleting the last section clears what the store still holds.
    public static BorderSideDto ToDto(BorderSide side) => new()
    {
        Sections = side.Sections.Items.Count == 0
            ? null
            : [.. side.Sections.Items.Select(s => new BorderSectionDto
            {
                From = s.From,
                To = s.To,
                Move = s.Move,
                MoveBlock = s.MoveBlock,
                Drag = s.Drag,
                DragBlock = s.DragBlock
            })]
    };

    public static MonitorDto ToDto(PhysicalMonitor monitor) => new()
    {
        XLocationInMm = monitor.DepthProjection.X,
        YLocationInMm = monitor.DepthProjection.Y,
        PhysicalRatioX = monitor.DepthRatio.X,
        PhysicalRatioY = monitor.DepthRatio.Y,
        BorderResistance = new BorderResistanceDto
        {
            Left = ToDto(monitor.BorderResistance.Left),
            Top = ToDto(monitor.BorderResistance.Top),
            Right = ToDto(monitor.BorderResistance.Right),
            Bottom = ToDto(monitor.BorderResistance.Bottom)
        },
        // Stored whatever the current mode is (they must survive a Save() made in
        // PerModel mode), but only once the monitor owns them: uncustomized monitors
        // keep mirroring the model and store nothing.
        Borders = monitor.BordersCustomized
            ? new BordersDto
            {
                Left = monitor.Borders.Left,
                Top = monitor.Borders.Top,
                Right = monitor.Borders.Right,
                Bottom = monitor.Borders.Bottom
            }
            : null,
        ActiveSource = monitor.ActiveSource.Source.Id,
        SerialNumber = monitor.SerialNumber,
        ExcludedFromLayout = monitor.ExcludedFromLayout,
        Sources = monitor.Sources.Items
            .Where(s => s.Source.AttachedToDesktop)
            .ToDictionary(s => s.Source.Id, s => new SourceDto
            {
                PixelX = s.Source.InPixel.X,
                PixelY = s.Source.InPixel.Y,
                PixelWidth = s.Source.InPixel.Width,
                PixelHeight = s.Source.InPixel.Height,
                Orientation = s.Source.Orientation,
                DisplayName = s.Source.DisplayName,
                Primary = s.Source.Primary
            })
    };

    public static ModelDto ToDto(PhysicalMonitorModel model) => new()
    {
        Width = model.PhysicalSize.Width,
        Height = model.PhysicalSize.Height,
        Borders = new BordersDto
        {
            Left = model.PhysicalSize.LeftBorder,
            Top = model.PhysicalSize.TopBorder,
            Right = model.PhysicalSize.RightBorder,
            Bottom = model.PhysicalSize.BottomBorder
        },
        PnpName = model.PnpDeviceName
    };
}
