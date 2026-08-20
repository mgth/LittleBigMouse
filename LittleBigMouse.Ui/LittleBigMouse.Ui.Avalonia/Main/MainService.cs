/*
  LittleBigMouse.Ui.Avalonia
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Ui.Avalonia.

    LittleBigMouse.Ui.Avalonia is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Ui.Avalonia is distributed in the hope that it will be useful,
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
using System.Reactive.Linq;
using System.Threading.Tasks;
using Avalonia.Controls;
using Avalonia.Threading;
using HLab.Base.ReactiveUI;
using HLab.Mvvm.Annotations;
using HLab.UserNotification;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Controls;
using LittleBigMouse.Platform.Windows;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Ui.Avalonia.Updater;
using LittleBigMouse.Zoning;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Main;

/// <summary>
/// The coordinator. It holds the layout the whole application edits, routes what the daemon and
/// the platform report, and decides what those reports <em>mean</em> — but it performs none of
/// it itself.
/// <para>
/// Five collaborators do the performing, and this class <em>owns</em> them rather than
/// resolving them: <see cref="DisplayChangeCoordinator"/> decides when a display change
/// deserves a rebuild, <see cref="EngineController"/> decides whether the mouse engine should
/// be hooked, <see cref="MainWindowManager"/> and <see cref="TrayIconController"/> put that
/// state in front of the user, and <see cref="WallpaperRefresher"/> keeps the drawn desktop
/// current. They are constructed here, wired to each other here, and released here, because
/// the wiring — a display change may rebuild, or may only re-hook; a resume must do both, in
/// that order — is the decision this class exists to make.
/// </para>
/// </summary>
public class MainService : ReactiveModel, IMainService
{
    readonly ILayoutFactory _layoutFactory;
    readonly ILayoutPersistence _layoutPersistence;
    readonly ILittleBigMouseClientService _littleBigMouseClientService;
    readonly IProcessesCollector _processesCollector;
    readonly Func<ApplicationUpdaterViewModel> _updaterLocator;

    readonly DisplayChangeCoordinator _displayChanges;
    readonly EngineController _engine;
    readonly MainWindowManager _windows;
    readonly TrayIconController _tray;

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
        _layoutFactory = layoutFactory;
        _layoutPersistence = layoutPersistence;
        _littleBigMouseClientService = littleBigMouseClientService;
        _processesCollector = processesCollector;
        _updaterLocator = updaterLocator;

        _engine = new EngineController(
            littleBigMouseClientService, layoutPersistence, () => MonitorsLayout);

        _displayChanges = new DisplayChangeCoordinator(
            layoutFactory.DisplaySignature,
            () => _engine.Suspended,
            UpdateLayout,
            _engine.StartIfEnabledAsync,
            _engine.EnsureHookedAsync);

        _windows = new MainWindowManager(mvvmService, mainViewModelLocator);
        _windows.DisposeWith(this);

        _tray = new TrayIconController(notify, options);
        _tray.DisposeWith(this);

        new WallpaperRefresher(
                layoutFactory, () => MonitorsLayout, action => Dispatcher.UIThread.Post(action))
            .DisposeWith(this);

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
                o => o.LoadAtStartup,
                o => o.StartElevated)
            .Skip(1)
            .Where(_ => !layoutPersistence.IsLoading)
            .Subscribe(_ => (MonitorsLayout as MonitorsLayout)?.UpdateSchedule())
            .DisposeWith(this);

        // Relate service state with the notify icon. Post to the UI thread so events arrive in
        // order (a Task.Run per event could invert Running/Stopped); the Safely wrapper keeps
        // one failed handler from stopping later events.
        void OnDaemonEvent(object? sender, LittleBigMouseServiceEventArgs args)
            => Dispatcher.UIThread.Post(() => _ = DaemonEventReceivedSafelyAsync(args));

        OwnedSubscription.Create<EventHandler<LittleBigMouseServiceEventArgs>>(
                OnDaemonEvent,
                h => littleBigMouseClientService.DaemonEventReceived += h,
                h => littleBigMouseClientService.DaemonEventReceived -= h)
            .DisposeWith(this);

        // Platforms without a daemon reporting display changes (Linux) detect them in the
        // factory itself; same debounce/settle/idempotence path as the daemon event.
        void OnDisplayChanged(object? sender, EventArgs args) => _ = _displayChanges.NotifyAsync();

        OwnedSubscription.Create<EventHandler>(
                OnDisplayChanged,
                h => layoutFactory.DisplayChanged += h,
                h => layoutFactory.DisplayChanged -= h)
            .DisposeWith(this);
    }

    public void UpdateLayout()
    {
        // TODO : move to plugin
        var old = MonitorsLayout;

        MonitorsLayout = (_virtualLayoutEnvDisabled ? null : VirtualLayoutEnvironment.Load())
                         ?? _layoutFactory.Create();

        old?.Dispose();
    }

    /// <summary>
    /// Sticky opt-out of LBM_VIRTUAL_LAYOUT: once the user explicitly comes back to the
    /// local layout, later rebuilds (display changes…) must not silently re-enter virtual
    /// mode — that trap is exactly what the badge's "back" button exists to avoid.
    /// </summary>
    bool _virtualLayoutEnvDisabled;

    public void ReloadSystemLayout()
    {
        _virtualLayoutEnvDisabled = true;
        UpdateLayout();
    }

    public Task ShowControlAsync()
    {
        _windows.Show(this, window => ConfirmLeavingAsync(window, leaving: false));
        return Task.CompletedTask;
    }

    /// <inheritdoc/>
    public bool LivePreview { get; set; }

    /// <summary>
    /// Is there anything to lose? The layout carries the flag for its whole subtree, so
    /// this is the same question the Save button answers.
    /// </summary>
    bool HasUnsavedLayout => MonitorsLayout is { IsVirtual: false, Saved: false };

    /// <summary>
    /// Stand between the user and leaving, but only when leaving costs something.
    /// Returns false when they chose to stay.
    /// </summary>
    async Task<bool> ConfirmLeavingAsync(Window? owner, bool leaving)
    {
        if (!HasUnsavedLayout) return true;

        var choice = await UnsavedChangesDialog.ShowAsync(owner, leaving, canSave: LivePreview);

        switch (choice)
        {
            case UnsavedChoice.Cancel:
                return false;
            case UnsavedChoice.Save:
                // Only reachable under live update, so this writes the geometry that is
                // driving the mouse at this moment — never one nobody has tried.
                await Task.Run(() => _layoutPersistence.Save((MonitorsLayout)MonitorsLayout));
                return true;
            default:
                return true;
        }
    }

    public async Task StartNotifierAsync()
    {
        // When a second instance launches, it signals the single-instance guard; show our
        // window instead of doing nothing. Raised on a background thread.
        if (Program.SingleInstance is { } singleInstance)
        {
            void OnShowRequested()
            {
                Console.Error.WriteLine("Single-instance activation requested: showing the control window.");
                Dispatcher.UIThread.Post(() => _ = ShowControlAsync());
            }

            OwnedSubscription.Create<Action>(
                    OnShowRequested,
                    h => singleInstance.ShowRequested += h,
                    h => singleInstance.ShowRequested -= h)
                .DisposeWith(this);
        }

        await _tray.InitializeAsync(new TrayMenu(
            // Hidden where the app cannot update itself (Linux: the distribution package
            // owns updates) — clicking it would be a no-op.
            CheckUpdateAsync: _updaterLocator().IsSupported
                ? () => _updaterLocator().CheckUpdateAsync(true)
                : null,
            OpenAsync: ShowControlAsync,
            StartAsync: _engine.StartFromUserAsync,
            StopAsync: _engine.StopFromUserAsync,
            RefreshAsync: _displayChanges.RefreshAsync,
            QuitAsync: QuitAsync));
    }

    public void AddControlPlugin(Action<IMainPluginsViewModel>? action) => _windows.AddPlugin(action);

    async Task QuitAsync()
    {
        // The one place the edits are actually lost.
        if (!await ConfirmLeavingAsync(_windows.Current, leaving: true)) return;

        // TODO : it should not append by sometimes the QuitAsync does not return
        _ = Task.Delay(5000).ContinueWith(
            _ => Dispatcher.UIThread.BeginInvokeShutdown(DispatcherPriority.Normal));
        await _littleBigMouseClientService.QuitAsync();
        Dispatcher.UIThread.BeginInvokeShutdown(DispatcherPriority.Normal);
    }

    /// <summary>
    /// The daemon reported something. The whole of this method is deciding what it means; the
    /// acting on it belongs to the collaborators.
    /// </summary>
    async Task DaemonEventReceivedAsync(LittleBigMouseServiceEventArgs args)
    {
        DaemonEventTrace.Write(args.Event);

        // Flags first, and synchronously: they gate paths that can run while the icon update
        // below is awaiting. Raising Suspended after an await would leave a window in which a
        // display change still believes there is a desktop.
        switch (args.Event)
        {
            // The daemon detected the display turning off (sleep / session standby / lock-idle)
            // and already unhooked itself, so the cursor is never left confined without us.
            // Stop reacting to display events until it comes back.
            case LittleBigMouseEvent.Suspended:
                _engine.Suspended = true;
                break;

            case LittleBigMouseEvent.Resumed:
                _engine.Suspended = false;
                break;

            case LittleBigMouseEvent.Connected:
                _justConnected = true;
                break;

            case LittleBigMouseEvent.Running:
                _justConnected = false;
                break;
        }

        await _tray.ShowDaemonStateAsync(args.Event);

        switch (args.Event)
        {
            // A daemon reporting itself stopped right after connecting has no layout yet: that
            // is not the user's Stop, it is a daemon waiting to be told what to do.
            case LittleBigMouseEvent.Stopped:
                if (MonitorsLayout is not null && MonitorsLayout.Options.Enabled && _justConnected)
                {
                    _justConnected = false;
                    await _engine.StartAsync();
                }
                break;

            case LittleBigMouseEvent.SettingsChanged:
            case LittleBigMouseEvent.DesktopChanged:
            case LittleBigMouseEvent.DisplayChanged:
                await _displayChanges.NotifyAsync();
                break;

            // Display is back: reconcile the layout only if it actually changed while off (the
            // idempotence guard), then keep re-hooking the daemon through the post-resume
            // display re-enumeration storm. A single Start loses a race — a late
            // WM_DISPLAYCHANGE unhooks us ~1-2s after Resumed — and the engine would stay
            // stopped ("blue") until a manual Start.
            case LittleBigMouseEvent.Resumed:
                await _displayChanges.NotifyAsync();
                await _engine.EnsureRunningAfterResumeAsync();
                break;

            case LittleBigMouseEvent.FocusChanged:
                _processesCollector?.AddProcess(args.Payload);
                break;

            // Load outcome: consumed by the location control (badge/status); nothing
            // to reconcile at the service level.
            case LittleBigMouseEvent.Loaded:
            case LittleBigMouseEvent.LoadFailed:
                break;

            // The panic shortcut ran. Its two steps announce themselves through the ordinary
            // events as well — a restore reloads, a stop unhooks — so the tray icon is already
            // right; the location control is what has to act.
            case LittleBigMouseEvent.Rescued:
                break;

            // Anything else, including events only a newer daemon knows about, is not ours to
            // reconcile. This used to throw, which faulted the handler on every Suspended,
            // Resumed and Probed.
            default:
                break;
        }
    }

    /// <summary>
    /// Set between a daemon connecting and its first Running: the Stopped that arrives in
    /// between means "I have no layout", not "the user stopped me".
    /// </summary>
    bool _justConnected;

    async Task DaemonEventReceivedSafelyAsync(LittleBigMouseServiceEventArgs args)
    {
        try
        {
            await DaemonEventReceivedAsync(args);
        }
        catch (Exception error)
        {
            // One failed notification must not stop later daemon events.
            Debug.WriteLine($"Daemon event handler failed: {error}");
        }
    }
}
