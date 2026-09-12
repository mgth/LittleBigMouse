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
using System.Reactive.Linq;
using System.Threading.Tasks;
using Avalonia.Controls;
using Avalonia.Threading;
using HLab.Base.ReactiveUI;
using HLab.Mvvm.Annotations;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Persistence;
using LittleBigMouse.Ui.Avalonia.Controls;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Ui.Avalonia.Updater;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Main;

/// <summary>
/// The window's side of the application (v6, phase 4). It holds the layout the whole
/// application edits, shows it, and keeps it in step with the displays.
/// <para>
/// What it no longer does is decide anything about the mouse engine. The agent owns the
/// hook, the profiles and the engine — when to rebuild, when to re-hook, what to write —
/// and this process is one of its frontends: it starts when the user opens it, and leaves
/// when they close it. What used to live here (the engine controller, the display-change
/// coordinator, the tray, the daemon's process management, the crash-recovery file) is the
/// agent's, and its rules are tested there.
/// </para>
/// <para>
/// Two collaborators are still owned here, because they are about the window rather than
/// the engine: <see cref="MainWindowManager"/> puts the layout in front of the user, and
/// <see cref="WallpaperRefresher"/> keeps the drawn desktop current.
/// </para>
/// </summary>
public class MainService : ReactiveModel, IMainService
{
    readonly ILayoutFactory _layoutFactory;
    readonly AgentClient _agent;
    readonly Action _leave;
    readonly Func<ApplicationUpdaterViewModel> _updaterLocator;

    readonly MainWindowManager _windows;

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
        AgentClient agent,
        ILayoutFactory layoutFactory,
        ILayoutPersistence layoutPersistence,
        Func<ApplicationUpdaterViewModel> updaterLocator,
        ILayoutOptions options)
        : this(mainViewModelLocator, mvvmService, agent, layoutFactory, layoutPersistence,
            updaterLocator, options, post => Dispatcher.UIThread.Post(() => post()),
            leave: () => Dispatcher.UIThread.BeginInvokeShutdown(DispatcherPriority.Normal))
    {
    }

    /// <summary>
    /// The dispatcher seam, injectable for the same reason <see cref="WallpaperRefresher"/>
    /// takes one: tests have no UI thread, and a post to a loop nobody pumps is a rebuild
    /// that never happens — which would make every assertion here pass for the wrong reason.
    /// </summary>
    internal MainService
    (
        Func<IMainPluginsViewModel> mainViewModelLocator,
        IMvvmService mvvmService,
        AgentClient agent,
        ILayoutFactory layoutFactory,
        ILayoutPersistence layoutPersistence,
        Func<ApplicationUpdaterViewModel> updaterLocator,
        ILayoutOptions options,
        Action<Action> postToUi,
        Action leave)
    {
        _leave = leave;
        _layoutFactory = layoutFactory;
        _agent = agent;
        _updaterLocator = updaterLocator;

        _windows = new MainWindowManager(mvvmService, mainViewModelLocator);
        _windows.DisposeWith(this);

        new WallpaperRefresher(layoutFactory, () => MonitorsLayout, postToUi)
            .DisposeWith(this);

        // App-level options never go through the engine start flow: they are recorded as
        // soon as they change instead of waiting for the save button (#406). They now go
        // to the agent, which is the only writer. The IsLoading guard keeps store loads
        // from echoing back.
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
            .Subscribe(_ => SaveOptions(options))
            .DisposeWith(this);

        // Starting with the session is the agent's too: it owns the autostart entry (the
        // XDG one on Linux, the scheduled task on Windows), so the option travels with the
        // others rather than being written here.
        options.WhenAnyValue(
                o => o.LoadAtStartup,
                o => o.StartElevated)
            .Skip(1)
            .Where(_ => !layoutPersistence.IsLoading)
            .Subscribe(_ => SaveOptions(options))
            .DisposeWith(this);

        // Platforms without a daemon reporting display changes (Linux) detect them in the
        // factory itself. The agent rebuilds its own layout on its own; this is the
        // window's copy, which has to follow what the user is looking at.
        void OnDisplayChanged(object? sender, EventArgs args) => postToUi(UpdateLayout);

        OwnedSubscription.Create<EventHandler>(
                OnDisplayChanged,
                h => layoutFactory.DisplayChanged += h,
                h => layoutFactory.DisplayChanged -= h)
            .DisposeWith(this);
    }

    /// <summary>The app-level options and the session autostart, recorded by the agent.</summary>
    void SaveOptions(ILayoutOptions options) => _ = FireAndForget(
        () => _agent.SaveOptionsAsync(
            LayoutDtoMapper.ToGlobalOptionsDto(options, null),
            [.. options.ExcludedList],
            options.LoadAtStartup),
        "Saving the options");

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
        // leaving: the window closing is the process leaving now, so the edits it holds
        // really are lost — there is no minimised app they come back to.
        _windows.Show(this, window => ConfirmLeavingAsync(window, leaving: true), onClosed: _leave);
        return Task.CompletedTask;
    }

    /// <inheritdoc/>
    public void CloseControl() => _windows.Current?.Close();

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
                // Only reachable under live update, so this keeps the geometry that is
                // driving the mouse at this moment — never one nobody has tried. The agent
                // writes it.
                if (MonitorsLayout is MonitorsLayout layout)
                    await _agent.SaveLayoutAsync(layout.Id, AgentDocument.Of(layout));
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

        // The tray lives in the agent now (it is there whether this window runs or not),
        // and so does the update check on the platforms that have one.
        await Task.CompletedTask;
    }

    public void AddControlPlugin(Action<IMainPluginsViewModel>? action) => _windows.AddPlugin(action);

    /// <summary>The updater, still the window's: the agent does not update anything.</summary>
    public Task CheckUpdateAsync() => _updaterLocator().IsSupported
        ? _updaterLocator().CheckUpdateAsync(true)
        : Task.CompletedTask;

    /// <summary>
    /// An asynchronous gesture nobody awaits has nowhere to report a failure to, and an
    /// unobserved exception would take the process down. Surviving it costs something: a
    /// failure now looks like nothing happening, so it goes to <see cref="Console.Error"/>,
    /// where the app's other diagnostics go and where a Release build still has it.
    /// </summary>
    internal static async Task FireAndForget(Func<Task> action, string what)
    {
        try
        {
            await action();
        }
        catch (Exception error)
        {
            Console.Error.WriteLine($"{what} failed: {error}");
        }
    }
}
