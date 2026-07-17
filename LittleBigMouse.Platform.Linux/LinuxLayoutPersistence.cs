#nullable enable
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Linux implementation of <see cref="ILayoutPersistence"/>: JSON files under
/// ~/.config/LittleBigMouse (XDG). The structure mirrors the Windows registry tree so
/// field semantics stay 1:1 with <c>PersistencyExtensions</c>:
/// <c>options.json</c> = the root key, <c>models.json</c> = the per-PnP "monitors" keys,
/// <c>layouts/&lt;id&gt;.json</c> = one "Layouts\{id}" key with its monitors and sources.
/// The excluded-process list stays a plain text file in the DATA dir — the daemon reads it.
/// Writes are atomic (temp file + rename).
/// </summary>
public class LinuxLayoutPersistence : ILayoutPersistence
{
    static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
    };

    public bool IsLoading { get; private set; }

    static string OptionsPath => Path.Combine(LbmPaths.ConfigDir, "options.json");
    static string ModelsPath => Path.Combine(LbmPaths.ConfigDir, "models.json");
    static string ExcludedPath => Path.Combine(LbmPaths.DataDir, "Excluded.txt");

    static string LayoutPath(IMonitorsLayout layout)
    {
        var id = string.Join("_", layout.Id.Split(Path.GetInvalidFileNameChars(), StringSplitOptions.RemoveEmptyEntries));
        return Path.Combine(LbmPaths.ConfigDir, "layouts", $"{id}.json");
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
            var options = ReadJson<GlobalOptionsDto>(OptionsPath);
            var models = ReadJson<Dictionary<string, ModelDto>>(ModelsPath);
            var saved = ReadJson<LayoutDto>(LayoutPath(layout));

            layout.Options.Elevated = Environment.IsPrivilegedProcess;

            ApplyGlobalOptions(layout.Options, options);
            LoadExcluded(layout.Options);
            ApplyLayoutOptions(layout.Options, saved?.Options);

            foreach (var monitor in layout.PhysicalMonitors)
            {
                if (models != null && models.TryGetValue(monitor.Model.PnpCode, out var model))
                    Apply(monitor.Model, model);

                if (saved != null && saved.Monitors.TryGetValue(monitor.Id, out var m))
                    Apply(monitor, m, layout.Options.BorderValues == "PerMonitor");

                // Mark the whole subtree saved even on a first run with no file yet
                // (the registry path does the same through GetOrSet's write-back).
                // The Saved propagation is TRANSITION-based (AutoRefresh/UnsavedOn):
                // a child left unsaved here never notifies again on later edits, and
                // the save button would never enable.
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

    static void ApplyGlobalOptions(ILayoutOptions o, GlobalOptionsDto? dto)
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

    static void ApplyLayoutOptions(ILayoutOptions o, LayoutOptionsDto? dto)
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

    void LoadExcluded(ILayoutOptions options)
    {
        options.ExcludedList.Clear();

        if (!File.Exists(ExcludedPath))
        {
            // First run: seed the default exclusions and write the file the daemon reads.
            foreach (var entry in ExcludedProcessDefaults.All) options.ExcludedList.Add(entry);
            try
            {
                Directory.CreateDirectory(LbmPaths.DataDir);
                File.WriteAllLines(ExcludedPath, options.ExcludedList);
            }
            catch { }
            return;
        }

        foreach (var line in File.ReadAllLines(ExcludedPath))
            options.ExcludedList.Add(line);
    }

    static void Apply(PhysicalMonitorModel model, ModelDto dto)
    {
        var fixedRatio = model.PhysicalSize.FixedAspectRatio;
        model.PhysicalSize.FixedAspectRatio = false;

        model.PhysicalSize.TopBorder = dto.Borders?.Top ?? model.PhysicalSize.TopBorder;
        model.PhysicalSize.RightBorder = dto.Borders?.Right ?? model.PhysicalSize.RightBorder;
        model.PhysicalSize.BottomBorder = dto.Borders?.Bottom ?? model.PhysicalSize.BottomBorder;
        model.PhysicalSize.LeftBorder = dto.Borders?.Left ?? model.PhysicalSize.LeftBorder;

        // A stored non-positive size must not override the freshly computed one.
        if (dto.Height is > 0) model.PhysicalSize.Height = dto.Height.Value;
        if (dto.Width is > 0) model.PhysicalSize.Width = dto.Width.Value;

        model.PhysicalSize.FixedAspectRatio = fixedRatio;

        if (!string.IsNullOrEmpty(dto.PnpName)) model.PnpDeviceName = dto.PnpName;
    }

    /// <summary>
    /// Flag the monitor and every savable child as saved. Runs after a load —
    /// with or without stored data — so that the next edit produces a
    /// true→false transition the reactive Saved chains can observe.
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

    static void Apply(PhysicalMonitor monitor, MonitorDto dto, bool perMonitorBorders)
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

        // Per-monitor bezel borders — only in "PerMonitor" mode ("PerModel" keeps them on
        // the shared model entry).
        if (perMonitorBorders && dto.Borders != null)
        {
            monitor.Borders.Left = dto.Borders.Left ?? monitor.Borders.Left;
            monitor.Borders.Top = dto.Borders.Top ?? monitor.Borders.Top;
            monitor.Borders.Right = dto.Borders.Right ?? monitor.Borders.Right;
            monitor.Borders.Bottom = dto.Borders.Bottom ?? monitor.Borders.Bottom;
        }
    }

    //==================//
    // Save             //
    //==================//

    public bool Save(MonitorsLayout layout)
    {
        SaveGlobalOptions(layout.Options);

        var dto = new LayoutDto
        {
            Options = new LayoutOptionsDto
            {
                AllowOverlaps = layout.Options.AllowOverlaps,
                AllowDiscontinuity = layout.Options.AllowDiscontinuity,
                Algorithm = layout.Options.Algorithm,
                MaxTravelDistance = layout.Options.MaxTravelDistance,
                FreelookCheckInterval = layout.Options.FreelookCheckInterval,
                FreelookEnabled = layout.Options.FreelookEnabled,
                LoopX = layout.Options.LoopX,
                LoopY = layout.Options.LoopY,
                Enabled = layout.Options.Enabled,
                AdjustPointer = layout.Options.AdjustPointer,
                AdjustSpeed = layout.Options.AdjustSpeed,
                Priority = layout.Options.Priority,
                PriorityUnhooked = layout.Options.PriorityUnhooked
            },
            Monitors = layout.PhysicalMonitors.ToDictionary(m => m.Id, m => ToDto(m, layout.Options.BorderValues == "PerMonitor"))
        };
        WriteJson(LayoutPath(layout), dto);

        var models = ReadJson<Dictionary<string, ModelDto>>(ModelsPath) ?? [];
        foreach (var monitor in layout.PhysicalMonitors)
        {
            var model = monitor.Model;
            models[model.PnpCode] = new ModelDto
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
            model.PhysicalSize.Saved = true;
            model.Saved = true;
        }
        WriteJson(ModelsPath, models);

        layout.Options.Saved = true;
        layout.Saved = true;
        return true;
    }

    static MonitorDto ToDto(PhysicalMonitor monitor, bool perMonitorBorders)
    {
        var dto = new MonitorDto
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
            Borders = perMonitorBorders
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
                    Primary = s.Source.Primary
                })
        };

        monitor.DepthProjection.Saved = true;
        monitor.DepthRatio.Saved = true;
        monitor.BorderResistance.Saved = true;
        foreach (var source in monitor.Sources.Items) source.Source.Saved = true;
        monitor.Saved = true;

        return dto;
    }

    public bool SaveEnabled(IMonitorsLayout layout)
    {
        var path = LayoutPath(layout);
        var dto = ReadJson<LayoutDto>(path) ?? new LayoutDto();
        dto.Options ??= new LayoutOptionsDto();
        dto.Options.Enabled = layout.Options.Enabled;
        WriteJson(path, dto);

        // Autostart (systemd user unit / .desktop entry) is not implemented on Linux yet.
        return true;
    }

    public void SaveLive(ILayoutOptions options) => SaveGlobalOptions(options);

    void SaveGlobalOptions(ILayoutOptions o)
    {
        WriteJson(OptionsPath, new GlobalOptionsDto
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
            HideTrayIcon = o.HideTrayIcon
        });

        try
        {
            Directory.CreateDirectory(LbmPaths.DataDir);
            File.WriteAllLines(ExcludedPath, o.ExcludedList);
        }
        catch { }
    }

    //==================//
    // JSON I/O         //
    //==================//

    static T? ReadJson<T>(string path) where T : class
    {
        try
        {
            if (!File.Exists(path)) return null;
            return JsonSerializer.Deserialize<T>(File.ReadAllText(path), JsonOptions);
        }
        catch
        {
            // A corrupt file must never prevent the app from starting: fall back to defaults.
            return null;
        }
    }

    static void WriteJson<T>(string path, T value)
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            var temp = path + ".tmp";
            File.WriteAllText(temp, JsonSerializer.Serialize(value, JsonOptions));
            File.Move(temp, path, overwrite: true);
        }
        catch { }
    }

    //==================//
    // DTOs             //
    //==================//

    class GlobalOptionsDto
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
    }

    class LayoutDto
    {
        public LayoutOptionsDto? Options { get; set; }
        public Dictionary<string, MonitorDto> Monitors { get; set; } = [];
    }

    class LayoutOptionsDto
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

    class MonitorDto
    {
        public double? XLocationInMm { get; set; }
        public double? YLocationInMm { get; set; }
        public double? PhysicalRatioX { get; set; }
        public double? PhysicalRatioY { get; set; }
        public BordersDto? BorderResistance { get; set; }
        public BordersDto? Borders { get; set; }
        public string? ActiveSource { get; set; }
        public string? SerialNumber { get; set; }
        public bool? ExcludedFromLayout { get; set; }
        public Dictionary<string, SourceDto>? Sources { get; set; }
    }

    class SourceDto
    {
        public double? PixelX { get; set; }
        public double? PixelY { get; set; }
        public double? PixelWidth { get; set; }
        public double? PixelHeight { get; set; }
        public int? Orientation { get; set; }
        public bool? Primary { get; set; }
    }

    class BordersDto
    {
        public double? Left { get; set; }
        public double? Top { get; set; }
        public double? Right { get; set; }
        public double? Bottom { get; set; }
    }

    class ModelDto
    {
        public double? Width { get; set; }
        public double? Height { get; set; }
        public BordersDto? Borders { get; set; }
        public string? PnpName { get; set; }
    }
}
