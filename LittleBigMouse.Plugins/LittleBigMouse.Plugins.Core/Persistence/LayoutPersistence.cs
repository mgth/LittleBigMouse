#nullable enable
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins.Persistence;

/// <summary>
/// Platform-neutral persistence engine: the whole model↔DTO mapping lives here, once.
/// A platform provides a dumb <see cref="ILayoutStore"/> (registry, JSON) plus the
/// autostart/elevation hooks, and inherits the complete <see cref="ILayoutPersistence"/>
/// behavior. Adding a persisted field means one DTO property and its two mapping lines in
/// this file — no per-OS code, so Windows/Linux can no longer drift apart.
/// </summary>
public abstract class LayoutPersistence : ILayoutPersistence
{
    readonly ILayoutStore _store;

    /// <summary>
    /// Excluded-defaults top-up version already applied, read from the store at load time
    /// and round-tripped into every global-options write (it is not part of the options
    /// model) so the one-time migration stays one-time.
    /// </summary>
    int? _excludedDefaultsVersion;

    protected LayoutPersistence(ILayoutStore store) => _store = store;

    public bool IsLoading { get; private set; }

    //==================//
    // Platform hooks   //
    //==================//

    /// <summary>Whether the current process runs elevated (administrator / root).</summary>
    protected virtual bool IsElevated => Environment.IsPrivilegedProcess;

    /// <summary>Whether the app is registered to start with the user session.</summary>
    protected virtual bool IsAutostartScheduled(IMonitorsLayout layout) => false;

    /// <summary>Align the session autostart with the options. No-op where not implemented (Linux).</summary>
    protected virtual void SetAutostart(IMonitorsLayout layout, bool enabled, bool elevated) { }

    /// <summary>
    /// Full path of the excluded-processes list. It is a plain-text FILE in the app data
    /// folder — the daemon reads it, so it stays out of <see cref="ILayoutStore"/>.
    /// Virtual so tests can redirect it.
    /// </summary>
    protected virtual string ExcludedListFile()
    {
        var dir = LbmPaths.DataDir;
        Directory.CreateDirectory(dir);

        var file = Path.Combine(dir, "Excluded.txt");
        // Self-heal: a buggy earlier version created "Excluded.txt" as a *directory*.
        if (Directory.Exists(file)) Directory.Delete(file, true);
        return file;
    }

    //==================//
    // Load             //
    //==================//

    public void Load(MonitorsLayout layout)
    {
        var wasLoading = IsLoading;
        IsLoading = true;
        try
        {
            var data = _store.Read(
                layout.Id,
                layout.PhysicalMonitors.Select(m => m.Model.PnpCode).Distinct().ToList());

            _excludedDefaultsVersion = data.GlobalOptions?.ExcludedDefaultsVersion;

            layout.Options.LoadAtStartup = IsAutostartScheduled(layout);
            layout.Options.Elevated = IsElevated;

            Apply(layout.Options, data.GlobalOptions);
            LoadExcluded(layout.Options, data.GlobalOptions);
            Apply(layout.Options, data.Layout?.Options);

            foreach (var monitor in layout.PhysicalMonitors)
            {
                if (data.Models.TryGetValue(monitor.Model.PnpCode, out var model))
                    Apply(monitor.Model, model);

                if (data.Layout != null && data.Layout.Monitors.TryGetValue(monitor.Id, out var m))
                    Apply(monitor, m);

                // Mark the whole subtree saved even on a first run with no stored data.
                // The Saved propagation is TRANSITION-based (AutoRefresh/UnsavedOn): a
                // child left unsaved here never notifies again on later edits, and the
                // save button would never enable.
                MarkSaved(monitor);
            }

            layout.Options.Saved = true;
            layout.Saved = true;
            layout.UpdatePhysicalMonitors();
        }
        finally
        {
            IsLoading = wasLoading;
        }
    }

    static void Apply(ILayoutOptions o, GlobalOptionsDto? dto)
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
        o.ShowMonitorActionWarning = dto.ShowMonitorActionWarning ?? o.ShowMonitorActionWarning;
        o.BorderValues = dto.BorderValues ?? o.BorderValues;
        o.HideTrayIcon = dto.HideTrayIcon ?? o.HideTrayIcon;
    }

    static void Apply(ILayoutOptions o, LayoutOptionsDto? dto)
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
        o.Priority = dto.Priority ?? o.Priority;
        o.PriorityUnhooked = dto.PriorityUnhooked ?? o.PriorityUnhooked;
    }

    static void Apply(PhysicalMonitorModel model, ModelDto dto)
    {
        var fixedRatio = model.PhysicalSize.FixedAspectRatio;
        model.PhysicalSize.FixedAspectRatio = false;

        model.PhysicalSize.TopBorder = dto.Borders?.Top ?? model.PhysicalSize.TopBorder;
        model.PhysicalSize.RightBorder = dto.Borders?.Right ?? model.PhysicalSize.RightBorder;
        model.PhysicalSize.BottomBorder = dto.Borders?.Bottom ?? model.PhysicalSize.BottomBorder;
        model.PhysicalSize.LeftBorder = dto.Borders?.Left ?? model.PhysicalSize.LeftBorder;

        // Versions predating the EDID-less size fallback persisted the bogus 0x0 GDI
        // placeholder for virtual displays (#419): a stored non-positive size must not
        // override the freshly computed one.
        if (dto.Height is > 0) model.PhysicalSize.Height = dto.Height.Value;
        if (dto.Width is > 0) model.PhysicalSize.Width = dto.Width.Value;

        model.PhysicalSize.FixedAspectRatio = fixedRatio;

        if (!string.IsNullOrEmpty(dto.PnpName)) model.PnpDeviceName = dto.PnpName;
    }

    static void Apply(PhysicalMonitor monitor, MonitorDto dto)
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

        monitor.BorderResistance.Left = dto.BorderResistance?.Left ?? monitor.BorderResistance.Left;
        monitor.BorderResistance.Top = dto.BorderResistance?.Top ?? monitor.BorderResistance.Top;
        monitor.BorderResistance.Right = dto.BorderResistance?.Right ?? monitor.BorderResistance.Right;
        monitor.BorderResistance.Bottom = dto.BorderResistance?.Bottom ?? monitor.BorderResistance.Bottom;

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

    /// <summary>
    /// Flag the monitor and every savable child as saved. Runs after a load — with or
    /// without stored data — so that the next edit produces a true→false transition the
    /// reactive Saved chains can observe.
    /// </summary>
    static void MarkSaved(PhysicalMonitor monitor)
    {
        monitor.Model.PhysicalSize.Saved = true;
        monitor.Model.Saved = true;

        monitor.DepthProjection.Saved = true;
        monitor.DepthRatio.Saved = true;
        monitor.BorderResistance.Saved = true;

        foreach (var source in monitor.Sources.Items)
        {
            source.Source.InPixel.Saved = true;
            source.Source.Saved = true;
            source.Saved = true;
        }

        monitor.Saved = true;
    }

    //==================//
    // Excluded list    //
    //==================//

    void LoadExcluded(ILayoutOptions options, GlobalOptionsDto? global)
    {
        options.ExcludedList.Clear();

        var file = ExcludedListFile();
        if (!File.Exists(file))
        {
            // First run: seed the defaults and write the file the daemon reads. The version
            // is remembered so the next global-options write records the top-up as applied.
            foreach (var entry in ExcludedProcessDefaults.All) options.ExcludedList.Add(entry);
            _excludedDefaultsVersion = ExcludedProcessDefaults.Version;
            try { File.WriteAllLines(file, options.ExcludedList); }
            catch { /* best effort: the in-memory list is seeded regardless */ }
            return;
        }

        foreach (var line in File.ReadAllLines(file)) options.ExcludedList.Add(line);

        MigrateExcludedDefaults(options.ExcludedList, global, file);
    }

    /// <summary>
    /// One-time top-up of the default exclusion list. When new default entries ship (e.g.
    /// Xbox game folders, #494) they must reach users who already have an
    /// <c>Excluded.txt</c> — a fresh seed only covers new installs. Runs once per
    /// <see cref="ExcludedProcessDefaults.Version"/> (tracked in the store) and only when
    /// the list still holds every previous default, so a customized list — or a default
    /// the user deliberately removed later — is left untouched. Rewrites the file too,
    /// since the daemon reads it, not this in-memory list.
    /// </summary>
    void MigrateExcludedDefaults(ICollection<string> list, GlobalOptionsDto? global, string file)
    {
        if ((_excludedDefaultsVersion ?? 0) >= ExcludedProcessDefaults.Version) return;

        // Only top up a list that still holds all the previous defaults (i.e. the user kept them).
        if (ExcludedProcessDefaults.LegacyV0.All(list.Contains))
        {
            var added = false;
            foreach (var entry in ExcludedProcessDefaults.All)
            {
                if (list.Contains(entry)) continue;
                list.Add(entry);
                added = true;
            }

            if (added)
            {
                try { File.WriteAllLines(file, list); }
                catch { /* best effort: the in-memory list is updated regardless */ }
            }
        }

        _excludedDefaultsVersion = ExcludedProcessDefaults.Version;
        var dto = global ?? new GlobalOptionsDto();
        dto.ExcludedDefaultsVersion = _excludedDefaultsVersion;
        _store.WriteGlobalOptions(dto);
    }

    //==================//
    // Save             //
    //==================//

    public bool Save(MonitorsLayout layout)
    {
        SetAutostart(layout, layout.Options.LoadAtStartup, layout.Options.StartElevated);

        SaveGlobalOptions(layout.Options);

        _store.WriteLayout(layout.Id, ToLayoutDto(layout));
        _store.WriteModels(layout.PhysicalMonitors
            .Select(m => m.Model)
            .DistinctBy(m => m.PnpCode)
            .ToDictionary(m => m.PnpCode, ToDto));

        foreach (var monitor in layout.PhysicalMonitors) MarkSaved(monitor);

        layout.Options.Saved = true;
        layout.Saved = true;
        return true;
    }

    public bool SaveEnabled(IMonitorsLayout layout)
    {
        // Read-modify-write: only Enabled changes; everything else stored for this layout
        // is preserved (the engine can be toggled on/off without a full save).
        var dto = _store.Read(layout.Id, []).Layout ?? new LayoutDto();
        dto.Options ??= new LayoutOptionsDto();
        dto.Options.Enabled = layout.Options.Enabled;
        _store.WriteLayout(layout.Id, dto);

        SetAutostart(layout, layout.Options.LoadAtStartup, layout.Options.StartElevated);
        return true;
    }

    public void SaveLive(ILayoutOptions options) => SaveGlobalOptions(options);

    void SaveGlobalOptions(ILayoutOptions o)
    {
        _store.WriteGlobalOptions(new GlobalOptionsDto
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
            ShowMonitorActionWarning = o.ShowMonitorActionWarning,
            BorderValues = o.BorderValues,
            HideTrayIcon = o.HideTrayIcon,
            ExcludedDefaultsVersion = _excludedDefaultsVersion
        });

        try { File.WriteAllLines(ExcludedListFile(), o.ExcludedList); }
        catch { /* best effort: the daemon re-reads it at the next full save */ }
    }

    static LayoutDto ToLayoutDto(MonitorsLayout layout) => new()
    {
        Options = ToDto(layout.Options),
        Monitors = layout.PhysicalMonitors.ToDictionary(m => m.Id, ToDto)
    };

    static LayoutOptionsDto ToDto(ILayoutOptions o) => new()
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
        AdjustSpeed = o.AdjustSpeed,
        Priority = o.Priority,
        PriorityUnhooked = o.PriorityUnhooked
    };

    static MonitorDto ToDto(PhysicalMonitor monitor) => new()
    {
        XLocationInMm = monitor.DepthProjection.X,
        YLocationInMm = monitor.DepthProjection.Y,
        PhysicalRatioX = monitor.DepthRatio.X,
        PhysicalRatioY = monitor.DepthRatio.Y,
        BorderResistance = new BordersDto
        {
            Left = monitor.BorderResistance.Left,
            Top = monitor.BorderResistance.Top,
            Right = monitor.BorderResistance.Right,
            Bottom = monitor.BorderResistance.Bottom
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

    static ModelDto ToDto(PhysicalMonitorModel model) => new()
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
