#nullable enable
using System;
using System.IO;
using System.Linq;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins.Persistence;

/// <summary>
/// Platform-neutral persistence engine, and the only entry point the UI knows. A platform
/// provides a dumb <see cref="ILayoutStore"/> (registry, JSON) plus the autostart/elevation
/// hooks, and inherits the complete <see cref="ILayoutPersistence"/> behavior — no per-OS
/// mapping code, so Windows/Linux can no longer drift apart.
/// <para>This class is the façade: what to load, in which order, and what counts as saved.
/// The three responsibilities it orchestrates live next door:</para>
/// <list type="bullet">
/// <item><see cref="LayoutDtoMapper"/> — the whole model↔DTO mapping, both directions;</item>
/// <item><see cref="LayoutMigrations"/> — how values written by older versions are read;</item>
/// <item><see cref="ExcludedListPersistence"/> — the excluded-processes file, its defaults
/// and their one-time top-up.</item>
/// </list>
/// </summary>
public abstract class LayoutPersistence : ILayoutPersistence
{
    readonly ILayoutStore _store;
    readonly ExcludedListPersistence _excluded;

    protected LayoutPersistence(ILayoutStore store)
    {
        _store = store;
        _excluded = new ExcludedListPersistence(store, ExcludedListFile);
    }

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

            layout.Options.LoadAtStartup = IsAutostartScheduled(layout);
            layout.Options.Elevated = IsElevated;

            // Global options first, then the per-layout ones: the layout overrides the app
            // level (Priority/PriorityUnhooked exist on both sides).
            LayoutDtoMapper.Apply(layout.Options, data.GlobalOptions);
            _excluded.Load(layout.Options, data.GlobalOptions);
            LayoutDtoMapper.Apply(layout.Options, data.Layout?.Options);

            foreach (var monitor in layout.PhysicalMonitors)
            {
                // Model before monitor: the monitor mapping reads the physical size the
                // model just restored (edge lengths, whole-edge resistance migration).
                if (data.Models.TryGetValue(monitor.Model.PnpCode, out var model))
                    LayoutDtoMapper.Apply(monitor.Model, model);

                if (data.Layout != null && data.Layout.Monitors.TryGetValue(monitor.Id, out var m))
                    LayoutDtoMapper.Apply(monitor, m);

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
    // Save             //
    //==================//

    public bool Save(MonitorsLayout layout)
    {
        if (RefuseVirtual(layout, nameof(Save))) return false;

        SetAutostart(layout, layout.Options.LoadAtStartup, layout.Options.StartElevated);

        SaveGlobalOptions(layout.Options);

        _store.WriteLayout(layout.Id, LayoutDtoMapper.ToLayoutDto(layout));
        _store.WriteModels(layout.PhysicalMonitors
            .Select(m => m.Model)
            .DistinctBy(m => m.PnpCode)
            .ToDictionary(m => m.PnpCode, LayoutDtoMapper.ToDto));

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

        // Everything else read is written back as-is, EXCEPT the two app-level options a
        // layout may still carry: re-emitting them here would put back what a full save
        // just migrated away (see LayoutDtoMapper.ToDto).
        dto.Options.Priority = null;
        dto.Options.PriorityUnhooked = null;

        _store.WriteLayout(layout.Id, dto);

        SetAutostart(layout, layout.Options.LoadAtStartup, layout.Options.StartElevated);
        return true;
    }

    public void SaveLive(ILayoutOptions options) => SaveGlobalOptions(options);

    void SaveGlobalOptions(ILayoutOptions o)
    {
        _store.WriteGlobalOptions(
            LayoutDtoMapper.ToGlobalOptionsDto(o, _excluded.AppliedDefaultsVersion));

        _excluded.Write(o.ExcludedList);
    }
}
