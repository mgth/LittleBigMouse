/*
  LittleBigMouse.Control.Core
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Control.Core.

    LittleBigMouse.Control.Core is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Control.Core is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reactive.Linq;
using System.Threading;
using System.Threading.Tasks;
using System.Threading.Channels;
using Avalonia.Controls;
using Avalonia.Threading;
using HLab.Base.Avalonia;
using HLab.Base.ReactiveUI;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Avalonia;
using HLab.UserNotification;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Options;
using LittleBigMouse.Platform.Windows;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Ui.Avalonia.Updater;
using LittleBigMouse.Zoning;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Main;

public class MainService : ReactiveModel, IMainService
{
    readonly Channel<(object? Sender, LittleBigMouseServiceEventArgs Args)> _daemonEvents =
        Channel.CreateUnbounded<(object?, LittleBigMouseServiceEventArgs)>(
            new UnboundedChannelOptions { SingleReader = true, SingleWriter = false });
    readonly IMvvmService _mvvmService;
    readonly ILayoutFactory _layoutFactory;
    readonly ILayoutPersistence _layoutPersistence;

    readonly IUserNotificationService _notify;
    readonly ILittleBigMouseClientService _littleBigMouseClientService;
    readonly IProcessesCollector _processesCollector;
    readonly Func<ApplicationUpdaterViewModel> _updaterLocator;
    readonly ILayoutOptions _options;


    Action<IMainPluginsViewModel>? _actions;

    readonly Func<IMainPluginsViewModel> _mainViewModelLocator;

    public IMonitorsLayout? MonitorsLayout 
    { 
        get => _monitorLayout; 
        set => this.RaiseAndSetIfChanged(ref _monitorLayout, value);
    }
    IMonitorsLayout? _monitorLayout;

    public MainService
    (
        Func<IMainPluginsViewModel> mainViewModelLocator,
        IMvvmService mvvmService,
        ILittleBigMouseClientService littleBigMouseClientService,
        IUserNotificationService notify,
        ILayoutFactory layoutFactory,
        ILayoutPersistence layoutPersistence,
        IProcessesCollector processesCollector,
        Func<ApplicationUpdaterViewModel> updaterLocator,
        ILayoutOptions options)
    {
        _notify = notify;
        _layoutFactory = layoutFactory;
        _layoutPersistence = layoutPersistence;
        _processesCollector = processesCollector;
        _updaterLocator = updaterLocator;
        _littleBigMouseClientService = littleBigMouseClientService;
        _options = options;

        _mvvmService = mvvmService;
        _mainViewModelLocator = mainViewModelLocator;

        // App-level options never go through the engine start flow: persist them as
        // soon as they change instead of waiting for the save button (#406). The
        // IsLoading guard keeps registry loads from echoing back.
        options.WhenAnyValue(
                o => o.AutoUpdate,
                o => o.StartMinimized,
                o => o.StartElevated,
                o => o.DebugTools,
                o => o.VcpControl,
                o => o.Pinned,
                o => o.ShowMonitorActionWarning)
            .Skip(1)
            .Where(_ => !layoutPersistence.IsLoading)
            .Subscribe(_ => layoutPersistence.SaveLive(options))
            .DisposeWith(this);

        options.WhenAnyValue(
                o => o.LoadAtStartup)
            .Skip(1)
            .Where(_ => !layoutPersistence.IsLoading)
            .Subscribe(_ => (MonitorsLayout as MonitorsLayout)?.UpdateSchedule())
            .DisposeWith(this);

        // Relate service state with notify icon
        _littleBigMouseClientService.DaemonEventReceived += (sender, args) =>
            _daemonEvents.Writer.TryWrite((sender, args));
        _ = ProcessDaemonEventsAsync();

        // The platform watches the OS for wallpaper changes (Windows: a RegNotifyChangeKeyValue
        // registry watcher) and raises WallpaperChanged. Refresh the wallpaper drawn behind each
        // monitor in place.
        _layoutFactory.WallpaperChanged += (_, _) => RefreshWallpaper();

        // Platforms without a daemon reporting display changes (Linux) detect them in the
        // factory itself; same debounce/settle/idempotence path as the daemon event.
        _layoutFactory.DisplayChanged += (_, _) => _ = DisplayChangedAsync();
    }

    public void UpdateLayout()
    {
        // TODO : move to plugin
        var old = MonitorsLayout;

        MonitorsLayout = _layoutFactory.Create();

        old?.Dispose();
    }

    Window _mainWindow = null!;

    public Task ShowControlAsync()
    {
        if (_mainWindow?.IsLoaded == true)
        {
            _mainWindow.WindowState = WindowState.Normal;
            _mainWindow.Activate();
            return Task.CompletedTask;
        }

        var viewModel = _mainViewModelLocator();
        viewModel.MainService = this;

        _actions?.Invoke(viewModel);

        var view = _mvvmService
            .MainContext
            .GetView<DefaultViewMode>(viewModel, typeof(IDefaultViewClass));

        _mainWindow = view?.AsWindow() ?? throw new Exception("No window found");

        // AsWindow() creates a bare DefaultWindow with no size: restore the last
        // session's geometry (or a sensible default) and save it back on close.
        MainWindowGeometry.Attach(_mainWindow);

        _mainWindow.Closed += (s, a) => _mainWindow = null!;

        _mainWindow.Show();

        return Task.CompletedTask;
    }


    public async Task StartNotifierAsync()
    {
        _notify.Click += async (s, a) => await ShowControlAsync();

        // When a second instance launches, it signals the single-instance guard; show our
        // window instead of doing nothing. Raised on a background thread.
        if (Program.SingleInstance is { } singleInstance)
        {
            singleInstance.ShowRequested += () => Dispatcher.UIThread.Post(() => _ = ShowControlAsync());
        }

        // Apply / react to the "hide tray icon" option. The notify service hides the tray icon
        // natively (Avalonia TrayIcon.IsVisible) — no per-OS code here.
        _options.PropertyChanged += (s, e) =>
        {
            if (e.PropertyName == nameof(ILayoutOptions.HideTrayIcon))
                _notify.Visible = !_options.HideTrayIcon;
        };

        await _notify.AddMenuAsync(-1, "Check for update","Icon/lbm_on", async () => await _updaterLocator().CheckUpdateAsync(true));
        await _notify.AddMenuAsync(-1, "Open","Icon/lbm_off", ShowControlAsync);
        await _notify.AddMenuAsync(-1, "Start","Icon/Start", StartAsync);
        await _notify.AddMenuAsync(-1, "Stop","Icon/Stop", () =>
        {
            MonitorsLayout.Options.Enabled = false;
            _layoutPersistence.SaveEnabled(MonitorsLayout);
            return _littleBigMouseClientService.StopAsync();
        });
        await _notify.AddMenuAsync(-1, "Refresh", "Icon/Refresh", RefreshAsync);
        await _notify.AddMenuAsync(-1, "Exit", "Icon/sys/Close", QuitAsync);

        // Apply the initial "hide tray icon" preference BEFORE loading the icon bitmap.
        // SetIconAsync queues a dispatcher lambda that checks _visible; if Visible=false
        // is already set here, the lambda returns early and NIM_ADD never fires.
        _notify.Visible = !_options.HideTrayIcon;

        await _notify.SetIconAsync("Icon/lbm_off",128);

        _notify.Show();
    }

    public void AddControlPlugin(Action<IMainPluginsViewModel>? action)
    {
        _actions += action;
    }

    async Task QuitAsync()
    {
        // TODO : it should not append by sometimes the QuitAsync does not return
        var t = Task.Delay(5000).ContinueWith(t => Dispatcher.UIThread.BeginInvokeShutdown(DispatcherPriority.Normal));
        await _littleBigMouseClientService.QuitAsync();
        Dispatcher.UIThread.BeginInvokeShutdown(DispatcherPriority.Normal);
    }

    Task StartAsync() => _littleBigMouseClientService.StartAsync(MonitorsLayout.ComputeZones());

    /// <summary>
    /// Tray-menu escape hatch: force a layout rebuild when the automatic display-change
    /// detection missed one (#443). Deliberately bypasses the idempotence guard, then
    /// realigns the built signature so the next display event doesn't rebuild again.
    /// </summary>
    async Task RefreshAsync()
    {
        // display off: nothing to rebuild against, the daemon's Resumed event reconciles
        if (_suspended) return;

        UpdateLayout();
        _lastBuiltSignature = _layoutFactory.DisplaySignature();
        if (MonitorsLayout.Options.Enabled)
            await StartAsync();
    }

    /// <summary>
    /// Rolling trace of daemon events: display-event storms (wake from sleep, HDR
    /// flapping...) rebuild the layout every 5s for hours, and identifying the
    /// looping event after the fact is otherwise impossible.
    /// </summary>
    static void TraceDaemonEvent(LittleBigMouseEvent evt)
    {
        try
        {
            var dir = LbmPaths.DataDir;
            Directory.CreateDirectory(dir);
            var file = Path.Combine(dir, "daemon-events.log");

            if (File.Exists(file) && new FileInfo(file).Length > 1_000_000)
                File.WriteAllText(file, "");

            File.AppendAllText(file, $"{DateTime.Now:yyyy-MM-dd HH:mm:ss.fff} {evt}\r\n");
        }
        catch
        {
            // tracing must never take the app down
        }
    }

    bool _justConnected = false;
    async Task ProcessDaemonEventsAsync()
    {
        await foreach (var item in _daemonEvents.Reader.ReadAllAsync())
        {
            var completion = new TaskCompletionSource(
                TaskCreationOptions.RunContinuationsAsynchronously);
            Dispatcher.UIThread.Post(async () =>
            {
                try
                {
                    await DaemonEventReceived(item.Sender, item.Args);
                    completion.SetResult();
                }
                catch (Exception error)
                {
                    completion.SetException(error);
                }
            });
            try
            {
                await completion.Task;
            }
            catch (Exception error)
            {
                // One failed notification must not permanently stop delivery of
                // every later daemon event.
                Debug.WriteLine($"Daemon event handler failed: {error}");
            }
        }
    }

    async Task DaemonEventReceived(object? sender, LittleBigMouseServiceEventArgs args)
    {
        TraceDaemonEvent(args.Event);

        switch (args.Event)
        {
            case LittleBigMouseEvent.Running:
                _justConnected = false;
                await _notify.SetIconAsync("icon/lbm_on",32);
                break;

            case LittleBigMouseEvent.Stopped:
                await _notify.SetIconAsync("icon/lbm_off",32);

                if (MonitorsLayout is not null && MonitorsLayout.Options.Enabled && _justConnected)
                {
                    _justConnected = false;
                    await StartAsync();
                }
                break;

            case LittleBigMouseEvent.Dead:
                await _notify.SetIconAsync("icon/lbm_dead",32);
                break;

            case LittleBigMouseEvent.Paused:
                await _notify.SetIconAsync("icon/lbm_paused",32);
                break;

            case LittleBigMouseEvent.SettingsChanged:
            case LittleBigMouseEvent.DesktopChanged:
            case LittleBigMouseEvent.DisplayChanged:
                await DisplayChangedAsync();
                break;

            // The daemon detected the display turning off (sleep / session standby / lock-idle) and
            // already unhooked itself (so the cursor is never left confined without us). Just stop
            // reacting to display events until it comes back — no rebuild while there is no desktop.
            case LittleBigMouseEvent.Suspended:
                _suspended = true;
                await _notify.SetIconAsync("icon/lbm_paused", 32);
                break;

            // Display is back: reconcile the layout only if it actually changed while off (the
            // idempotence guard), then keep re-hooking the daemon through the post-resume display
            // re-enumeration storm. A single StartAsync loses a race — a late WM_DISPLAYCHANGE
            // unhooks us ~1-2s after Resumed — and the engine stays stopped ("blue") until a manual
            // Start. EnsureRunningAfterResumeAsync re-asserts Start until it sticks.
            case LittleBigMouseEvent.Resumed:
                _suspended = false;
                await DisplayChangedAsync();
                await EnsureRunningAfterResumeAsync();
                break;

            case LittleBigMouseEvent.FocusChanged:
                _processesCollector?.AddProcess(args.Payload);
                break;
            case LittleBigMouseEvent.Connected:
                _justConnected = true;
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(args.Event), args.Event, null);
        }
    }

    // React to a display change (attach/detach, resolution, scaling, primary) once the OS has
    // actually SETTLED, without a fixed delay. The daemon forwards a burst of
    // WM_DISPLAYCHANGE/WM_SETTINGCHANGE per change, so we:
    //   1. trailing-debounce the burst (a generation counter: only the latest event proceeds),
    //   2. confirm the config has stopped changing (two identical DisplaySignature reads),
    // then rebuild. Measured on real switches, the config reaches its final state within one
    // ~80ms sample, so this reacts in a few hundred ms instead of the former fixed 5s.
    const int DisplayDebounceMs = 300;      // quiet window absorbing the message burst
    const int DisplayStabilityStepMs = 100; // spacing between settle re-checks
    const int DisplayStabilityMaxSteps = 8; // ~1.1s cap so a flapping config can't hang the settle loop
    int _displayChangeGeneration;

    async Task DisplayChangedAsync()
    {
        // Do nothing at all while the display is off: no debounce, no signature reads, no rebuild.
        // The daemon's Resumed event reconciles once when the desktop is back.
        if (_suspended) return;

        var gen = Interlocked.Increment(ref _displayChangeGeneration);

        // Trailing debounce: wait a short quiet window; if a newer event arrived meanwhile, bail
        // and let that later call handle it (so a burst collapses to a single rebuild).
        await Task.Delay(DisplayDebounceMs);
        if (gen != Volatile.Read(ref _displayChangeGeneration)) return;

        // Settle detection: rebuild only once the display config has stopped changing, i.e. two
        // consecutive signatures match. Capped so a pathological flapping config can't hang here.
        var signature = _layoutFactory.DisplaySignature();
        for (var i = 0; i < DisplayStabilityMaxSteps; i++)
        {
            await Task.Delay(DisplayStabilityStepMs);
            if (gen != Volatile.Read(ref _displayChangeGeneration)) return;
            var next = _layoutFactory.DisplaySignature();
            if (next == signature) break;
            signature = next;
        }

        // A Suspended event may have arrived during the debounce/settle (the display went off while
        // this display-change was being handled): drop the rebuild — the daemon has unhooked and we
        // wait for Resumed.
        if (_suspended) return;

        // Idempotence guard: the settle loop confirms the config STOPPED changing, but not that it
        // actually DIFFERS from the generation we already built. A spurious WM_DISPLAYCHANGE (a
        // monitor's DPMS power-save on/off — e.g. an HDR TV —, a mode re-apply, a stray broadcast)
        // settles to the very same signature we last built. Rebuilding it drops a fresh, fully-wired
        // MonitorsLayout generation that Avalonia's compositor/animation clock keeps alive (#412): a
        // storm of identical events is what fills gigabytes. Skip when nothing changed.
        if (signature == _lastBuiltSignature)
        {
            // Skip the REBUILD, but still make sure the engine is hooked. The daemon unhooks itself
            // over ANY display change (so the cursor is never confined while the desktop reshapes);
            // when the config settles back to the same signature, nothing else would re-Start it, so
            // the engine would stay stopped ("blue") until a manual Start. Re-hook without rebuilding.
            await EnsureEngineHookedAsync();
            return;
        }

        UpdateLayout();
        _lastBuiltSignature = signature;
        if (MonitorsLayout.Options.Enabled)
            await StartAsync();
    }

    // The daemon reports Running only while its low-level mouse hook is actually installed, so its
    // published State is the source of truth for "is the engine hooked right now".
    bool EngineRunning => _littleBigMouseClientService.State == LittleBigMouseEvent.Running;

    /// <summary>
    /// Re-hook the daemon if the engine should be running but is not — WITHOUT rebuilding the layout.
    /// Called from the idempotence guard when a display change settles to the already-built config:
    /// the daemon unhooks itself over any display change, and when the config comes back identical
    /// nothing else re-Starts it. Safe while an excluded app (a game) is focused — the daemon's Run
    /// path no-ops when paused, so this can never force the hook on over an exclusion.
    /// </summary>
    async Task EnsureEngineHookedAsync()
    {
        if (_resumeWatchdogActive) return;                        // the resume watchdog owns reconciliation
        if (_suspended) return;                                   // display off — nothing to do
        if (!(MonitorsLayout?.Options.Enabled ?? false)) return;  // engine disabled by the user
        if (EngineRunning) return;                                // already hooked
        await StartAsync();
    }

    /// <summary>
    /// Keep the engine hooked across the display re-enumeration storm that follows a wake from sleep.
    /// The daemon emits Resumed as soon as the screen turns on, but Windows then re-enumerates the
    /// GPU/monitors for several seconds, firing a burst of WM_DISPLAYCHANGE. One of those, processed
    /// just after we re-install the hook, makes the daemon unhook itself again (a Stopped ~1-2s after
    /// Resumed), and a single StartAsync loses that race — the engine stays stopped ("blue") until a
    /// manual Start. Re-assert Start until it sticks (Running for a short quiet window), bounded so a
    /// legitimately-paused engine (excluded app focused at wake) can't loop forever.
    /// </summary>
    async Task EnsureRunningAfterResumeAsync()
    {
        if (!(MonitorsLayout?.Options.Enabled ?? false)) return;

        var myGen = Interlocked.Increment(ref _resumeGeneration);
        _resumeWatchdogActive = true;
        try
        {
            var restarts = 0;
            var quiet = 0;
            for (var i = 0; i < ResumeWatchdogMaxSteps; i++)
            {
                if (myGen != Volatile.Read(ref _resumeGeneration)) return; // a newer resume superseded us
                if (_suspended) return;                                    // went back to sleep
                if (!(MonitorsLayout?.Options.Enabled ?? false)) return;   // user disabled the engine

                if (EngineRunning)
                {
                    if (++quiet >= ResumeWatchdogStableSteps) return; // Running long enough — converged
                }
                else if (restarts >= ResumeWatchdogMaxRestarts)
                {
                    return; // gave up — the engine is legitimately paused (excluded app focused at wake)
                }
                else
                {
                    quiet = 0;
                    restarts++;
                    await StartAsync();
                }

                await Task.Delay(ResumeWatchdogStepMs);
            }
        }
        finally
        {
            _resumeWatchdogActive = false;
        }
    }

    const int ResumeWatchdogStepMs = 500;      // spacing between engine-state checks
    const int ResumeWatchdogStableSteps = 3;   // consecutive Running checks that count as converged (~1.5s)
    const int ResumeWatchdogMaxRestarts = 6;   // cap re-Starts so a paused/excluded engine can't loop forever
    const int ResumeWatchdogMaxSteps = 60;     // hard stop (~30s) against a pathological flapping wake

    // Guards the post-resume watchdog: a generation counter so a newer Resumed supersedes an older
    // watchdog run, and a flag telling the idempotence-guard reconcile to stand aside while the
    // watchdog is the active driver. Written on the daemon-event path — volatile is enough.
    int _resumeGeneration;
    volatile bool _resumeWatchdogActive;

    // Signature of the display configuration the current MonitorsLayout was built from. Lets
    // DisplayChangedAsync skip a rebuild when a display event settles to an already-built config.
    string _lastBuiltSignature = "";

    // Set while the display is off (daemon Suspended/Resumed events): DisplayChangedAsync
    // short-circuits while set, so the UI does nothing while there is no desktop. Written on the
    // daemon-event thread — volatile is enough (a single flag).
    volatile bool _suspended;

    /// <summary>
    /// Refresh the wallpaper drawn behind each monitor after the registry watcher reports a change.
    /// Runs the in-place read on the UI thread; <see cref="ILayoutFactory.UpdateWallpaper"/> gates on
    /// a cheap signature, so the several registry writes Windows makes per change collapse to a
    /// single actual refresh, and unchanged notifications cost almost nothing.
    /// </summary>
    void RefreshWallpaper()
    {
        Dispatcher.UIThread.Post(() =>
        {
            if (MonitorsLayout is MonitorsLayout layout) _layoutFactory.UpdateWallpaper(layout);
        });
    }

}
