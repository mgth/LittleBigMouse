using System.ComponentModel;
using System.Reactive.Linq;
using System.Reactive.Subjects;
using HLab.ColorTools;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Wallpaper;
using LittleBigMouse.Plugins;
using ReactiveUI;
using SixLabors.ImageSharp;

namespace LittleBigMouse.Plugin.Wallpaper.Avalonia;

/// <summary>
/// Shared state behind every WallpaperFrameView: settings for the current layout,
/// debounced live apply, and automatic span re-slice when the layout is rebuilt
/// (display change) or saved (screens moved in the editor). Never reads the OS
/// wallpaper state back (DisplaySource.WallpaperPath stays a preview-only mirror),
/// which is what keeps the apply → watcher → editor-refresh loop harmless.
/// </summary>
public class WallpaperManager : ReactiveObject, IDisposable
{
    readonly IMainService _mainService;
    readonly IWallpaperService _service;
    readonly Dictionary<string, LayoutWallpaperSettings> _all;
    readonly Subject<bool> _applyRequests = new();
    readonly IDisposable _applySubscription;

    IMonitorsLayout? _layout;
    string? _lastAppliedSignature;

    public WallpaperManager(IMainService mainService, IWallpaperService service)
    {
        _mainService = mainService;
        _service = service;
        _all = WallpaperSettingsStore.Load();

        _applySubscription = _applyRequests
            .Throttle(TimeSpan.FromMilliseconds(300))
            .Subscribe(x => _ = ApplyAsync());

        if (!service.IsSupported) return;

        if (mainService is INotifyPropertyChanged notifier)
            notifier.PropertyChanged += OnMainServiceChanged;
        if (mainService.MonitorsLayout is { } layout)
            AttachLayout(layout);
    }

    void OnMainServiceChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(IMainService.MonitorsLayout)) return;
        if (_mainService.MonitorsLayout is { } layout) AttachLayout(layout);
    }

    /// <summary>A rebuilt layout: reload its settings and re-slice onto the new topology.</summary>
    void AttachLayout(IMonitorsLayout layout)
    {
        if (ReferenceEquals(_layout, layout)) return;

        if (_layout is INotifyPropertyChanged oldNotifier)
            oldNotifier.PropertyChanged -= OnLayoutChanged;

        _layout = layout;
        if (layout is INotifyPropertyChanged newNotifier)
            newNotifier.PropertyChanged += OnLayoutChanged;

        this.RaisePropertyChanged(nameof(Mode));
        this.RaisePropertyChanged(nameof(SpanImagePath));

        if (Current?.HasContent == true) RequestApply();
    }

    /// <summary>Screens moved in the editor: re-slice on save, not on every drag.</summary>
    void OnLayoutChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(IMonitorsLayout.Saved)) return;
        if (_layout?.Saved != true) return;
        if (Current is { Mode: WallpaperMode.Span, HasContent: true }) RequestApply();
    }

    LayoutWallpaperSettings? Current
        => _layout != null && _all.TryGetValue(_layout.Id, out var settings) ? settings : null;

    LayoutWallpaperSettings CurrentForEdit
    {
        get
        {
            var id = _layout?.Id ?? "";
            if (!_all.TryGetValue(id, out var settings))
                _all[id] = settings = new LayoutWallpaperSettings();
            return settings;
        }
    }

    public WallpaperMode Mode
    {
        get => Current?.Mode ?? WallpaperMode.PerScreen;
        set
        {
            if (Mode == value) return;
            CurrentForEdit.Mode = value;
            this.RaisePropertyChanged();
            Commit();
        }
    }

    public string? SpanImagePath
    {
        get => Current?.SpanImagePath;
        set
        {
            if (SpanImagePath == value) return;
            CurrentForEdit.SpanImagePath = value;
            this.RaisePropertyChanged();
            Commit();
        }
    }

    public ScreenWallpaperSettings GetScreen(string monitorId)
    {
        if (!CurrentForEdit.PerScreen.TryGetValue(monitorId, out var settings))
            CurrentForEdit.PerScreen[monitorId] = settings = new ScreenWallpaperSettings();
        return settings;
    }

    /// <summary>Persist the settings and push them to the desktop (debounced).</summary>
    public void Commit()
    {
        WallpaperSettingsStore.Save(_all);
        RequestApply();
    }

    void RequestApply() => _applyRequests.OnNext(true);

    async Task ApplyAsync()
    {
        try
        {
            if (!_service.IsSupported) return;
            var layout = _layout;
            var settings = Current;
            if (layout == null || settings is not { HasContent: true }) return;

            var screens = await Task.Run(() => BuildScreens(layout, settings));
            if (screens.Count == 0) return;

            // Mirror of MainService's _lastBuiltSignature: an unchanged target set
            // (same paths — slices are content-addressed — same styles/colors) is
            // already on screen, don't rewrite appletsrc for nothing.
            var signature = string.Join(";", screens.Select(s =>
                $"{s.LogicalBounds}|{s.ImagePath}|{s.Style}|{s.Color?.ToUInt()}"));
            if (signature == _lastAppliedSignature) return;
            _lastAppliedSignature = signature;

            await _service.ApplyAsync(screens);
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"Wallpaper apply failed: {e}");
        }
    }

    /// <summary>Pure: settings + layout → per-screen targets. Public for harness/tests.</summary>
    public static List<ScreenWallpaper> BuildScreens(IMonitorsLayout layout, LayoutWallpaperSettings settings)
    {
        var monitors = layout.PhysicalMonitors
            .Where(m => m.ActiveSource?.Source.AttachedToDesktop == true)
            .ToList();

        return settings.Mode == WallpaperMode.Span
            ? BuildSpanScreens(monitors, settings)
            : BuildPerScreenScreens(monitors, settings);
    }

    static List<ScreenWallpaper> BuildPerScreenScreens(List<PhysicalMonitor> monitors, LayoutWallpaperSettings settings)
    {
        var screens = new List<ScreenWallpaper>();
        foreach (var monitor in monitors)
        {
            if (!settings.PerScreen.TryGetValue(monitor.Id, out var screen)) continue;
            var bounds = monitor.ActiveSource.Source.InPixel.Bounds;

            if (screen.Kind == ScreenWallpaperKind.Image)
            {
                if (screen.ImagePath is { Length: > 0 } path && File.Exists(path))
                    screens.Add(new ScreenWallpaper(bounds, path, screen.Style, null));
            }
            else
            {
                screens.Add(new ScreenWallpaper(bounds, null, screen.Style, ParseColor(screen.Color)));
            }
        }
        return screens;
    }

    static List<ScreenWallpaper> BuildSpanScreens(List<PhysicalMonitor> monitors, LayoutWallpaperSettings settings)
    {
        if (settings.SpanImagePath is not { Length: > 0 } path || !File.Exists(path)) return [];

        var boundsMm = WallpaperSpanSlicer.ComputeBoundsMm(
            monitors.Select(m => m.DepthProjection.OutsideBounds));

        var inputs = monitors.Select(m =>
        {
            var source = m.ActiveSource.Source;
            // InPixel is the cursor space: native panel pixels on Windows, compositor
            // logical size on Linux where EffectiveDpi = 96 * scale recovers the panel.
            var kx = OperatingSystem.IsWindows() ? 1.0 : source.EffectiveDpi.X / 96;
            var ky = OperatingSystem.IsWindows() ? 1.0 : source.EffectiveDpi.Y / 96;
            var outputPx = new HLab.Geo.Size(
                Math.Max(1, Math.Round(source.InPixel.Width * kx)),
                Math.Max(1, Math.Round(source.InPixel.Height * ky)));
            return new WallpaperSpanSlicer.ScreenInput(m.Id, m.DepthProjection.Bounds, outputPx);
        }).ToList();

        var info = Image.Identify(path);
        var slices = WallpaperSpanSlicer.ComputeSlices(
            new HLab.Geo.Size(info.Width, info.Height), boundsMm, inputs);

        var files = SpanRenderer.Render(path, slices);

        // Each slice already has the exact screen aspect: Stretch maps it 1:1.
        return monitors
            .Where(m => files.ContainsKey(m.Id))
            .Select(m => new ScreenWallpaper(
                m.ActiveSource.Source.InPixel.Bounds, files[m.Id], WallpaperStyle.Stretch, null))
            .ToList();
    }

    internal static ColorRGB<double>? ParseColor(string hex)
    {
        var value = hex.TrimStart('#');
        if (value.Length != 6 || !uint.TryParse(value, System.Globalization.NumberStyles.HexNumber, null, out var rgb))
            return null;
        return HLabColors.RGB(1.0,
            ((rgb >> 16) & 0xFF) / 255.0,
            ((rgb >> 8) & 0xFF) / 255.0,
            (rgb & 0xFF) / 255.0);
    }

    public void Dispose()
    {
        if (_mainService is INotifyPropertyChanged notifier)
            notifier.PropertyChanged -= OnMainServiceChanged;
        if (_layout is INotifyPropertyChanged layoutNotifier)
            layoutNotifier.PropertyChanged -= OnLayoutChanged;
        _applySubscription.Dispose();
        _applyRequests.Dispose();
    }
}
