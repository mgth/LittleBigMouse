using System.Windows.Input;
using DynamicData;
using HLab.Geo;
using HLab.UserNotification;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The world <c>MainService</c> and its collaborators talk to, reduced to what can be asserted
/// on: a daemon that records commands and can be told what state to report, a store that
/// records writes, a tray that records icons, and a display configuration that can be moved by
/// hand.
/// </summary>
static class MainServiceFakes
{
    /// <summary>
    /// A layout that is real — <c>ComputeZones</c> walks it for every Start — but minimal: one
    /// monitor, one source, attached.
    /// </summary>
    public static MonitorsLayout NewLayout(
        ILayoutOptions options, bool enabled = true, LayoutSource source = LayoutSource.System)
    {
        var layout = new MonitorsLayout(options) { Id = "TESTMON1", Source = source };
        layout.Options.Enabled = enabled;

        var model = new PhysicalMonitorModel("TST1234");
        model.PhysicalSize.Width = 600;
        model.PhysicalSize.Height = 340;

        var monitor = new PhysicalMonitor("MON1", layout, model);
        var displaySource = new DisplaySource("SRC1") { AttachedToDesktop = true };
        displaySource.InPixel.Set(new Rect(new Point(0, 0), new Size(1920, 1080)));

        var physicalSource = new PhysicalSource("DEV1", monitor, displaySource);
        monitor.ActiveSource = physicalSource;
        monitor.Sources.Add(physicalSource);

        layout.AddOrUpdatePhysicalMonitor(monitor);
        layout.AddOrUpdatePhysicalSource(physicalSource);
        return layout;
    }
}

/// <summary>
/// The daemon, as the UI can see it: a command log and a state it publishes. Nothing here
/// hooks anything — <see cref="State"/> is set by the test to say whether the hook took.
/// </summary>
sealed class FakeDaemon : ILittleBigMouseClientService
{
    public List<string> Commands { get; } = [];
    public LittleBigMouseEvent State { get; set; } = LittleBigMouseEvent.Stopped;

    /// <summary>Number of Start commands after which the daemon starts reporting Running.</summary>
    public int StartsBeforeItSticks { get; set; }

    public int StartCount => Commands.Count(c => c == "Start");

    public event EventHandler<LittleBigMouseServiceEventArgs>? DaemonEventReceived;

    public void Raise(LittleBigMouseEvent daemonEvent, string payload = "")
        => DaemonEventReceived?.Invoke(this, new LittleBigMouseServiceEventArgs(daemonEvent, payload));

    /// <summary>True while anything is still subscribed — what a released subscription must undo.</summary>
    public bool HasSubscribers => DaemonEventReceived is not null;

    public Task StartAsync(ZonesLayout zonesLayout, CancellationToken token = default)
    {
        Commands.Add("Start");
        if (StartCount >= StartsBeforeItSticks) State = LittleBigMouseEvent.Running;
        return Task.CompletedTask;
    }

    public Task StopAsync(CancellationToken token = default)
    {
        Commands.Add("Stop");
        State = LittleBigMouseEvent.Stopped;
        return Task.CompletedTask;
    }

    public Task QuitAsync(CancellationToken token = default)
    {
        Commands.Add("Quit");
        return Task.CompletedTask;
    }

    public Task SendLiveAsync(ZonesLayout zonesLayout, CancellationToken token = default)
    {
        Commands.Add("Live");
        return Task.CompletedTask;
    }

    public Task SendShortcutAsync(string shortcut, CancellationToken token = default)
    {
        Commands.Add("Shortcut");
        return Task.CompletedTask;
    }
}

/// <summary>The store, reduced to what was written to it.</summary>
sealed class FakePersistence : ILayoutPersistence
{
    public bool IsLoading { get; set; }
    public int EnabledWrites { get; private set; }
    public bool? LastEnabledWritten { get; private set; }
    public int LiveOptionWrites { get; private set; }

    /// <summary>Full layout saves — the geometry, not just the engine's on/off.</summary>
    public int LayoutWrites { get; private set; }

    public void Load(MonitorsLayout layout) { }

    public bool Save(MonitorsLayout layout)
    {
        LayoutWrites++;
        return true;
    }

    public bool SaveEnabled(IMonitorsLayout layout)
    {
        EnabledWrites++;
        LastEnabledWritten = layout.Options.Enabled;
        return true;
    }

    public void SaveLive(ILayoutOptions options) => LiveOptionWrites++;
}

/// <summary>The tray icon, reduced to the icons it was asked to show and the menu it was given.</summary>
sealed class FakeNotification : IUserNotificationService
{
    public List<string> Icons { get; } = [];
    public List<string> MenuHeaders { get; } = [];
    public string ToolTipText { get; set; } = "";
    public bool Visible { get; set; } = true;
    public bool Shown { get; private set; }

    public string? LastIcon => Icons.Count > 0 ? Icons[^1] : null;

    public event Action<object, object>? Click;

    public void RaiseClick() => Click?.Invoke(this, EventArgs.Empty);

    /// <summary>True while anything is still subscribed — what a released subscription must undo.</summary>
    public bool HasClickSubscribers => Click is not null;

    public Task AddMenuAsync(int pos, string header, string icon, Func<Task> todo)
    {
        MenuHeaders.Add(header);
        Menu[header] = todo;
        return Task.CompletedTask;
    }

    public Task AddMenuAsync(int pos, string header, string icon, ICommand todo)
    {
        MenuHeaders.Add(header);
        return Task.CompletedTask;
    }

    public Dictionary<string, Func<Task>> Menu { get; } = [];

    public Task SetIconAsync(string icon, int i)
    {
        Icons.Add(icon);
        return Task.CompletedTask;
    }

    public void Show() => Shown = true;
}

/// <summary>
/// The platform's view of the displays: a signature the test moves by hand, a layout it hands
/// out on request, and the two events the platform raises on its own.
/// </summary>
sealed class FakeLayoutFactory(Func<MonitorsLayout> create) : ILayoutFactory
{
    public string Signature { get; set; } = "one-monitor";
    public int Creations { get; private set; }
    public int WallpaperUpdates { get; private set; }

    public MonitorsLayout Create()
    {
        Creations++;
        return create();
    }

    public string DisplaySignature() => Signature;

    public event EventHandler? DisplayChanged;
    public event EventHandler? WallpaperChanged;

    public void RaiseDisplayChanged() => DisplayChanged?.Invoke(this, EventArgs.Empty);
    public void RaiseWallpaperChanged() => WallpaperChanged?.Invoke(this, EventArgs.Empty);

    /// <summary>True while anything is still subscribed — what a released subscription must undo.</summary>
    public bool HasDisplaySubscribers => DisplayChanged is not null;
    public bool HasWallpaperSubscribers => WallpaperChanged is not null;

    public void UpdateWallpaper(MonitorsLayout layout) => WallpaperUpdates++;
}
