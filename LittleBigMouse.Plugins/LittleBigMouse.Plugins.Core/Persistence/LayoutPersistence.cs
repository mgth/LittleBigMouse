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
    // Virtual guard    //
    //==================//

    /// <summary>
    /// The single choke point keeping virtual (foreign) layouts out of the local store
    /// and the autostart scheduling. Guarding here rather than at the call sites means
    /// no UI path — current or future — can leak a client's configuration into this
    /// machine's state.
    /// </summary>
    static bool RefuseVirtual(IMonitorsLayout layout, string operation)
    {
        if (!layout.IsVirtual) return false;
        Console.Error.WriteLine(
            $"[LittleBigMouse] {operation} refused: '{layout.Id}' is a virtual layout ({layout.SourceOrigin ?? layout.Source.ToString()})");
        return true;
    }

    //==================//
    // Load             //
    //==================//

    public void Load(MonitorsLayout layout)
    {
        // A virtual layout's state comes from its export, and only from it: reading the
        // local store here would apply options persisted for whatever LOCAL layout shares
        // the client's id — foreign data over foreign data, all of it wrong.
        if (RefuseVirtual(layout, nameof(Load))) return;

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
        o.VcpControl = dto.VcpControl ?? o.VcpControl;
        o.ShowMonitorActionWarning = dto.ShowMonitorActionWarning ?? o.ShowMonitorActionWarning;
        o.BorderValues = dto.BorderValues ?? o.BorderValues;
        o.RescueShortcut = dto.RescueShortcut ?? o.RescueShortcut;
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
        if (dto is { Height: > 0, Width: > 0 })
        {
            // Migration of pre-5.4.1 oriented stored sizes (#507) — see NormalizeStoredSize.
            var (w, h) = NormalizeStoredSize(
                model.PhysicalSize.Width, model.PhysicalSize.Height,
                dto.Width.Value, dto.Height.Value);
            model.PhysicalSize.Width = w;
            model.PhysicalSize.Height = h;
        }
        else
        {
            if (dto.Height is > 0) model.PhysicalSize.Height = dto.Height.Value;
            if (dto.Width is > 0) model.PhysicalSize.Width = dto.Width.Value;
        }

        model.PhysicalSize.FixedAspectRatio = fixedRatio;

        if (!string.IsNullOrEmpty(dto.PnpName)) model.PnpDeviceName = dto.PnpName;
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

    static void Apply(BorderSide side, BorderSideDto? dto, double edgeLengthMm)
    {
        if (dto == null) return;

        side.Sections.Edit(list =>
        {
            list.Clear();

            if (dto.Sections is { } stored)
            {
                foreach (var s in stored)
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

                if (list.Count > 0) return;
            }

            // An edge saved before the section editor carried a single resistance
            // over its whole length. Rather than keep that notion alongside the
            // sections, it becomes the section that says the same thing — so an
            // existing setting stays in force AND shows up in the editor, where it
            // can be split or trimmed like any other.
            var legacy = LegacySection(dto, edgeLengthMm);
            if (legacy != null) list.Add(legacy);
        });
    }

    static BorderSection? LegacySection(BorderSideDto dto, double edgeLengthMm)
    {
        if (edgeLengthMm <= 0) return null;

        var move = dto.Move ?? 0;
        var drag = dto.Drag ?? 0;
        var moveBlock = dto.MoveBlock ?? false;
        var dragBlock = dto.DragBlock ?? false;

        // A zero, unblocked edge is what "no resistance" has always looked like:
        // migrating it would litter every layout with meaningless full-edge sections.
        if (move <= 0 && drag <= 0 && !moveBlock && !dragBlock) return null;

        return new BorderSection
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

        // Depth first: marking a side saved after its sections would be undone by
        // the collection's own unsaved propagation.
        foreach (var side in (BorderSide[])
                 [
                     monitor.BorderResistance.Left, monitor.BorderResistance.Top,
                     monitor.BorderResistance.Right, monitor.BorderResistance.Bottom
                 ])
        {
            foreach (var section in side.Sections.Items) section.Saved = true;
            side.Saved = true;
        }

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
        // Separator-insensitive: pre-V2 Linux lists were seeded with the Windows-style entries.
        if (ExcludedProcessDefaults.LegacyV0.All(e => ExcludedProcessDefaults.ContainsEntry(list, e)))
        {
            var added = false;
            foreach (var entry in ExcludedProcessDefaults.All)
            {
                if (ExcludedProcessDefaults.ContainsEntry(list, entry)) continue;
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
        if (RefuseVirtual(layout, nameof(Save))) return false;

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
        // This is the guard that matters most: SaveEnabled runs on every Stop, so without
        // it a virtual layout CREATES a store entry named after the client's monitor
        // combination (observed in the wild as a 61-byte {"Enabled": false} file).
        if (RefuseVirtual(layout, nameof(SaveEnabled))) return false;

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
            VcpControl = o.VcpControl,
            ShowMonitorActionWarning = o.ShowMonitorActionWarning,
            BorderValues = o.BorderValues,
            RescueShortcut = o.RescueShortcut,
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

    // Sections only: the per-edge resistance is gone, and writing it back would
    // resurrect it on the next load through the migration path. Always emitted, even
    // empty, so deleting the last section clears what the store still holds.
    static BorderSideDto ToDto(BorderSide side) => new()
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

    static MonitorDto ToDto(PhysicalMonitor monitor) => new()
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
