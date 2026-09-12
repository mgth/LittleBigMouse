using System.ComponentModel;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Wallpaper.Avalonia;

/// <summary>
/// Shared state behind every WallpaperFrameView: the settings of the layout being shown,
/// and the one gesture that leaves this process — handing them to the agent.
/// <para>
/// It used to write <c>wallpaper.json</c> and paint the desktop itself (v5). Both are the
/// agent's now: it cuts the span slices and sets the background, so a display change puts
/// the wallpaper back whether or not this window is open — which it usually is not. What
/// is left here is the editor's own state.
/// </para>
/// </summary>
public class WallpaperManager : ReactiveObject, IDisposable
{
    readonly IMainService _mainService;
    readonly IAgentWallpaper _agent;
    readonly Dictionary<string, LayoutWallpaperSettings> _all;

    IMonitorsLayout? _layout;

    public WallpaperManager(IMainService mainService, IAgentWallpaper agent)
    {
        _mainService = mainService;
        _agent = agent;
        // Read, never written: the agent is the writer, and this is what it wrote.
        _all = WallpaperSettingsStore.Load();

        if (mainService is INotifyPropertyChanged notifier)
            notifier.PropertyChanged += OnMainServiceChanged;
        if (mainService.MonitorsLayout is { } layout) AttachLayout(layout);
    }

    void OnMainServiceChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(IMainService.MonitorsLayout)) return;
        if (_mainService.MonitorsLayout is { } layout) AttachLayout(layout);
    }

    /// <summary>A rebuilt layout: show its settings.</summary>
    void AttachLayout(IMonitorsLayout layout)
    {
        if (ReferenceEquals(_layout, layout)) return;

        _layout = layout;

        this.RaisePropertyChanged(nameof(Mode));
        this.RaisePropertyChanged(nameof(SpanImagePath));
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

    /// <summary>
    /// Hand the settings to the agent, which records them and paints the desktop. The
    /// answer is not waited for: an agent that refuses leaves the editor as it is, and
    /// the wallpaper as it was — which is what a failed apply always looked like.
    /// </summary>
    public void Commit()
    {
        if (_layout is not { } layout) return;
        var settings = CurrentForEdit;
        _ = Send();
        return;

        async Task Send()
        {
            try
            {
                await _agent.SaveWallpaperAsync(layout.Id, settings);
            }
            catch (Exception error)
            {
                // Nobody awaits this, so there is nowhere to report to but the log the
                // app's other diagnostics go to. Letting it out would be an unobserved
                // exception raised on a thread that has nothing to do with the editor.
                Console.Error.WriteLine($"Saving the wallpaper failed: {error.Message}");
            }
        }
    }

    /// <summary>
    /// Whether a colour is finished being typed: exactly "#RRGGBB". The editor commits on
    /// every keystroke, so a half-typed colour must not be sent — it would repaint the
    /// screen black on the way to the value the user means.
    /// </summary>
    public static bool IsColor(string hex)
    {
        var value = hex.TrimStart('#');
        return value.Length == 6
               && uint.TryParse(value, System.Globalization.NumberStyles.HexNumber, null, out _);
    }

    public void Dispose()
    {
        if (_mainService is INotifyPropertyChanged notifier)
            notifier.PropertyChanged -= OnMainServiceChanged;
    }
}
