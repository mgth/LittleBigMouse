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
using System.IO;
using System.Reactive;
using System.Reactive.Concurrency;
using System.Reactive.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Tasks;
using System.Windows.Input;
using Avalonia.Threading;
using DynamicData;
using HLab.Base;
using HLab.Base.Avalonia;
using HLab.Base.Avalonia.Extensions;
using HLab.Base.ReactiveUI;
using HLab.Mvvm.ReactiveUI;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Persistence;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Zoning;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Controls;

public class LocationControlViewModel : ViewModel<MonitorsLayout>, ISavable
{
    readonly ISystemMonitorsService _monitorsService;
    readonly IMainService _mainService;
    readonly ILayoutPersistence _persistence;

    readonly AgentClient _agent;
    readonly Action<Action> _postToUi;

    public LocationControlViewModel(AgentClient agent, IMainService main, ISystemMonitorsService monitorsService, ILayoutPersistence persistence)
        : this(agent, main, monitorsService, persistence,
            run => Dispatcher.UIThread.Invoke(run),
            post => Dispatcher.UIThread.Post(() => post()),
            tick => new DispatcherLiveTicker(tick))
    {
    }

    /// <summary>
    /// The dispatcher seams, injectable for the same reason <see cref="DaemonStatusTracker"/>
    /// takes its <c>onUiThread</c>: tests have no UI thread, and without a platform Avalonia's
    /// dispatcher belongs to whichever thread reaches it first — a blocking Invoke from any
    /// other one waits forever on a loop nobody pumps.
    /// </summary>
    internal LocationControlViewModel(AgentClient agent, IMainService main,
        ISystemMonitorsService monitorsService, ILayoutPersistence persistence,
        Action<Action> onUiThread, Action<Action> postToUi, Func<Func<Task>, ILiveTicker> liveTicker)
    {
        _agent = agent;
        _mainService = main;
        _persistence = persistence;
        _postToUi = postToUi;

        _monitorsService = monitorsService;

        // What the daemon says about itself — see DaemonStatusTracker. Built before the commands
        // below, whose canExecute reads Running and Dead through the helpers wired here.
        _status = new DaemonStatusTracker(
            onUiThread,
            () => LiveUpdate,
            wasPreviewing =>
            {
                LiveUpdate = false;
                if (wasPreviewing) _ = AbandonPreviewAsync();
            });

        _running = _status.WhenAnyValue(e => e.Running)
            .ToProperty(this, e => e.Running, scheduler: Scheduler.Immediate);

        _dead = _status.WhenAnyValue(e => e.Dead)
            .ToProperty(this, e => e.Dead, scheduler: Scheduler.Immediate);

        _daemonLayoutInfo = _status.WhenAnyValue(e => e.LayoutInfo)
            .ToProperty(this, e => e.DaemonLayoutInfo, scheduler: Scheduler.Immediate);

        SaveCommand = ReactiveCommand.CreateFromTask(
            SaveAsync, 
            this.WhenAnyValue(e => e.Model.Saved, 
            selector: saved  => !saved)
                .ObserveOn(RxSchedulers.MainThreadScheduler));

        UndoCommand = ReactiveCommand.CreateFromTask(
            LoadAsync,
            this.WhenAnyValue(e => e.Model.Saved,selector: s => !s)
            .ObserveOn(RxSchedulers.MainThreadScheduler));

        StartCommand = ReactiveCommand.CreateFromTask(
            StartAsync,
            this.
                WhenAnyValue(
                e => e.Running,
                e => e.Dead,
                e => e.Model.Saved,
                e => e.Model,
                // A virtual layout can always be (re)sent for simulation: Running
                // refers to the SYSTEM engine and Saved is meaningless here.
                (running, dead, saved, model) => (model?.IsVirtual == true || !running || !saved) && !dead)
                .ObserveOn(RxSchedulers.MainThreadScheduler));

        StopCommand = ReactiveCommand.CreateFromTask(
            StopAsync,
            this.WhenAnyValue(e => e.Running)
        );

        // Virtual-layout surface: the badge, its origin tooltip, the "simulate"
        // wording and the way back. All derive from the (immutable per instance)
        // source of the current model.
        _isVirtualLayout = this.WhenAnyValue(e => e.Model)
            .Select(m => m?.IsVirtual ?? false)
            .ToProperty(this, e => e.IsVirtualLayout);

        _virtualLayoutOrigin = this.WhenAnyValue(e => e.Model)
            .Select(m => m?.IsVirtual != true ? ""
                : m.SourceOrigin is { Length: > 0 } origin
                    ? $"Foreign layout, shown for inspection: {origin}"
                    : "Foreign layout, shown for inspection")
            .ToProperty(this, e => e.VirtualLayoutOrigin);

        // The click means the same thing in both modes — hand the layout to the engine
        // and keep it — so only the second line has to say what the mode adds.
        _startTooltip = this.WhenAnyValue(e => e.Model)
            .Select(m => m?.IsVirtual == true ? "Simulate" : "Apply and start")
            .ToProperty(this, e => e.StartTooltip);

        _startTooltipDetail = this.WhenAnyValue(e => e.Model, e => e.LiveUpdate)
            .Select(t => t.Item1?.IsVirtual == true
                ? "Send this layout to the daemon for validation. The input hook stays off — a foreign layout never drives the local mouse."
                : t.Item2
                    ? "Live update is on: your changes are already being felt, but none of them is saved. This keeps them."
                    : "Save the layout and hand it to the mouse engine. Use the arrow to have changes sent as you make them instead.")
            .ToProperty(this, e => e.StartTooltipDetail);

        BackToLocalLayoutCommand = ReactiveCommand.Create(
            _mainService.ReloadSystemLayout,
            this.WhenAnyValue(e => e.Model)
                .Select(m => m?.IsVirtual ?? false)
                .ObserveOn(RxSchedulers.MainThreadScheduler));

        // What live preview needs is a daemon to talk to, not a running engine — its own
        // sends are what puts the engine up. Keying this on Running would be circular:
        // a Load unhooks the daemon before the Run behind it hooks again, so Running
        // dips on every tick and the switch would turn itself off a fifth of a second
        // after being clicked. A foreign layout is excluded outright: the daemon refuses
        // to hook one, so there would be nothing to feel.
        // The two are not looking at the same desktop. It happens when one of them has
        // not noticed a display change yet — and it matters, because everything this view
        // sends names the layout it was built for, so the agent refuses all of it. The
        // banner says so before the user finds out by pressing a button.
        _agentLayoutMismatch = this
            .WhenAnyValue(e => e.AgentLayoutId, e => e.Model, e => e.IsVirtualLayout)
            .Select(t => t is { Item3: false, Item1: { Length: > 0 } theirs, Item2: { } mine }
                         && mine.Id != theirs)
            .ToProperty(this, e => e.AgentLayoutMismatch);

        // What it is for: the agent rebuilds from the displays it can see now (#443).
        RefreshCommand = ReactiveCommand.CreateFromTask(
            () => Ask(() => _agent.RefreshAsync(), "Refresh"),
            this.WhenAnyValue(e => e.Dead, dead => !dead)
                .ObserveOn(RxSchedulers.MainThreadScheduler));

        _canLiveUpdate = this
            .WhenAnyValue(e => e.Dead, e => e.IsVirtualLayout, (dead, virtualLayout) => !dead && !virtualLayout)
            .ToProperty(this, e => e.CanLiveUpdate);

        this.WhenAnyValue(e => e.CanLiveUpdate)
            .Where(can => !can)
            .Subscribe(_ => LiveUpdate = false);

        // Two modes, each one its own command, so the menu can show both at once with
        // the current one marked. A single toggling entry read as a control that was
        // off rather than as the mode you are not in.
        ManualUpdateCommand = ReactiveCommand.Create(() => { LiveUpdate = false; });

        LiveUpdateCommand = ReactiveCommand.Create(
            () => { LiveUpdate = true; },
            this.WhenAnyValue(e => e.CanLiveUpdate).ObserveOn(RxSchedulers.MainThreadScheduler));

        // The engine going down while we preview into it — the tray Stop, a display
        // change, an excluded application — outranks the preview: the next tick would
        // otherwise hook it straight back up. A transition, not the current state:
        // turning the switch on over a stopped engine is a legitimate way to start.
        this.WhenAnyValue(e => e.Running)
            .Skip(1)
            .Where(running => !running)
            .Subscribe(_ => LiveUpdate = false);

        _live = new LiveLayoutUpdater(
            () => SavableReactiveModel.Revision,
            () => Model,
            (layout, token) => _agent.PreviewAsync(layout.Id, AgentDocument.Of(layout), token));

        _liveTimer = liveTicker(() => _live.TickAsync());

        this.WhenAnyValue(e => e.LiveUpdate).Subscribe(live =>
        {
            // Told to the service because leaving is decided there, and because only
            // live update makes an unsaved layout safe to offer to save: it is the one
            // driving the mouse, so it has been felt.
            _mainService.LivePreview = live;

            if (live)
            {
                // The daemon is holding the last applied layout, not ours: make the
                // first tick send unconditionally.
                _live.Forget();
                _liveTimer.Start();
            }
            else _liveTimer.Stop();
        });

        this.UnsavedOn(e => e.Model);

        // The agent's client is a process-lifetime singleton: a bare += would keep every
        // generation of this view model alive for as long as the app runs. Disposal
        // happens when the owning view leaves the logical tree (HLab.Mvvm's LinkDispose).
        OwnedSubscription.Create<EventHandler<LittleBigMouseServiceEventArgs>>(
                (_, e) => _status.Apply(e),
                h => agent.HookEventReceived += h,
                h => agent.HookEventReceived -= h)
            .DisposeWith(this);

        // The engine's own state travels in the agent's snapshot, not in a hook event: the
        // hook reports what it did (a load, a probe, a rescue), the agent reports what it
        // is. Without this the view would sit on whatever it last inferred — a Running that
        // the tray, another frontend or a display change has since stopped.
        OwnedSubscription.Create<EventHandler<AgentState>>(
                (_, state) =>
                {
                    _status.Apply(new(state.EngineEvent, ""));
                    _postToUi(() => AgentLayoutId = state.LayoutId);
                },
                h => agent.StateChanged += h,
                h => agent.StateChanged -= h)
            .DisposeWith(this);

        // No agent answering is the same thing to this view as no hook: nothing to drive.
        OwnedSubscription.Create<EventHandler<bool>>(
                (_, connected) =>
                {
                    if (!connected) _status.Apply(new(LittleBigMouseEvent.Dead, ""));
                },
                h => agent.ConnectionChanged += h,
                h => agent.ConnectionChanged -= h)
            .DisposeWith(this);

        _status.Apply(new LittleBigMouseServiceEventArgs(
            agent.State?.EngineEvent ?? LittleBigMouseEvent.Dead, ""));
        AgentLayoutId = agent.State?.LayoutId;
    }

    public override void OnDispose()
    {
        // Ends a live preview with its owner: LivePreview goes back to the service and
        // the timer stops — a running DispatcherTimer is rooted by the dispatcher, and
        // its tick reaches the daemon through this view model.
        LiveUpdate = false;
        _liveTimer.Stop();

        // Every WhenAnyValue chain watching through Model (command canExecute, UnsavedOn)
        // is subscribed to the layout itself, which lives on MainService: letting go of
        // the model is what unhooks them.
        Model = null;
    }

    readonly DaemonStatusTracker _status;

    protected override MonitorsLayout? OnModelChanging(MonitorsLayout? oldModel, MonitorsLayout? newModel)
    {
        // The last Load outcome belongs to the previous layout generation. Runs before the
        // field is assigned on the very first model, where there is nothing to forget.
        _postToUi?.Invoke(() => _status?.ForgetLayoutInfo());

        // A rebuild (display change, refresh, virtual layout opened) goes through
        // MainService, which feeds the daemon itself: what we believe it holds no longer
        // follows from what we last sent. Runs before the field is assigned on the very
        // first model, where there is nothing to forget anyway.
        _live?.Forget();

        if (newModel is { } model)
        {
            model.PhysicalMonitors.AsObservableChangeSet()
                .ObserveOn(RxSchedulers.MainThreadScheduler)
                .WhenValueChanged(e => e.Saved)
                .Do(e =>
                {
                    if (!e) Saved = false;
                }).Subscribe().DisposeWith(model);

            model.PhysicalMonitors.AsObservableChangeSet()
                .ObserveOn(RxSchedulers.MainThreadScheduler)
                .WhenValueChanged(e => e.Model.Saved)
                .Do(e =>
                {
                    if (!e) Saved = false;
                }).Subscribe().DisposeWith(model);

            model.PhysicalSources.AsObservableChangeSet()
                .ObserveOn(RxSchedulers.MainThreadScheduler)
                .WhenValueChanged(e => e.Saved)
                .Do(e =>
                {
                    if (!e) Saved = false;
                }).Subscribe().DisposeWith(model);

        }

        return base.OnModelChanging(oldModel, newModel);
    }

    public class JsonExport(DisplayDevice devices, MonitorsLayout? layout, ZonesLayout? zones)
    {
        public MonitorsLayout? Layout { get; } = layout;
        public DisplayDevice Devices { get; } = devices;
        public ZonesLayout? Zones { get; } = zones;
    }

    public string ExportConfig()
    {
        if (Model == null) return "";

        // _monitorsService.Root lazily triggers the Win32 device enumeration — Windows-only
        // (the export's Devices section has no Linux equivalent yet).
        if (!OperatingSystem.IsWindows()) return "";

        var export = new JsonExport(_monitorsService.Root, Model, Model?.ComputeZones());

        var json = JsonSerializer.Serialize(export, new JsonSerializerOptions{NumberHandling = JsonNumberHandling.AllowNamedFloatingPointLiterals});

        return json;
    }
    
    /// <summary>
    /// Displays a layout rebuilt from an export, for debugging purpose:
    /// allows inspecting another user's layout without the hardware.
    /// </summary>
    public void OpenVirtualLayout(string json)
    {
        _mainService.MonitorsLayout = VirtualLayoutFactory.FromJson(json, LayoutSource.VirtualImport);
    }

    public ReactiveCommand<Unit, Unit> SaveCommand { get; }
    public ReactiveCommand<Unit, Unit> UndoCommand { get; }
    public ReactiveCommand<Unit, Unit> StartCommand { get; }
    public ReactiveCommand<Unit, Unit> StopCommand { get; }

    /// <summary>
    /// "Apply and start" — the same gesture as the tray's Start, so it is the same code:
    /// <see cref="EngineController"/> owns what starting means (the virtual-layout
    /// simulation, recording Enabled, handing the zones over). What the button adds is
    /// <c>keepLayout</c>: this one comes from an editor, so the geometry on screen is kept
    /// too.
    /// </summary>
    /// <summary>
    /// "Apply and start": the edit goes to the agent, which applies it, writes it and hooks
    /// — one gesture, one message. The agent records Enabled itself, as C#'s engine
    /// controller did before it.
    /// </summary>
    async Task StartAsync()
    {
        if (Model is not { } layout) return;
        if (layout.IsVirtual)
        {
            // A foreign layout is inspected, never adopted: the agent takes no document
            // for one. Simulating it is not in the API yet (phase 4 note).
            Console.Error.WriteLine("A foreign layout cannot be simulated through the agent yet.");
            return;
        }
        if (await Ask(() => _agent.StartEngineAsync(layout.Id, AgentDocument.Of(layout)), "Start"))
            _postToUi(() => LayoutPersistence.MarkLayoutSaved(layout));
    }

    async Task StopAsync()
    {
        // Asking for the engine to stop outranks previewing into it — otherwise the
        // next tick would hook it straight back up. The agent records the user's Stop.
        LiveUpdate = false;

        await Ask(() => _agent.StopEngineAsync(), "Stop");
    }

    /// <summary>
    /// One request to the agent, with its refusal shown rather than thrown. A command that
    /// lets an exception out has nowhere to report it — ReactiveUI would take it to the
    /// dispatcher, where the user sees a log line at best — and the layout is not lost by
    /// a request that did not happen, so the honest answer is to say so where the load
    /// outcome is already shown and leave the model as it is.
    /// </summary>
    /// <returns>True when the agent did it.</returns>
    async Task<bool> Ask(Func<Task> request, string what)
    {
        try
        {
            await request();
            return true;
        }
        catch (Exception error) when (error is AgentException
                                      or OperationCanceledException
                                      or IOException)
        {
            var reason = error is OperationCanceledException ? "no agent answered" : error.Message;
            Console.Error.WriteLine($"{what} refused: {reason}");
            _status.Say($"{what} refused: {reason}");
            return false;
        }
    }

    /// <summary>
    /// The Save button: the agent writes, this side only learns that the model is no
    /// longer dirty (v6: one writer).
    /// </summary>
    async Task SaveAsync()
    {
        if (Model is not { } layout || layout.Saved) return;
        if (await Ask(() => _agent.SaveLayoutAsync(layout.Id, AgentDocument.Of(layout)), "Save"))
            _postToUi(() => LayoutPersistence.MarkLayoutSaved(layout));
    }

    /// <summary>
    /// The panic shortcut interrupted a live preview: throw the experiment away and put
    /// the engine back on the layout the user actually saved. Nothing is written — the
    /// preview never was — so this is a pure reload plus a fresh start.
    /// </summary>
    async Task AbandonPreviewAsync()
    {
        await LoadAsync();
        if (Model is null || Model.IsVirtual) return;
        // The agent puts the engine back on the layout it holds — the saved one.
        await Ask(() => _agent.EndPreviewAsync(), "Ending the preview");
    }

    Task LoadAsync() =>
        Task.Run(() =>
        {
            if (!(Model?.Saved??true))
                _persistence.Load(Model);
        });




    public bool Running => _running.Value;
    readonly ObservableAsPropertyHelper<bool> _running;

    public bool Dead => _dead.Value;
    readonly ObservableAsPropertyHelper<bool> _dead;

    /// <summary>
    /// How the apply button delivers: on click, or continuously. While it is on, every
    /// edit reaches the running daemon within <see cref="LiveLayoutUpdater.Interval"/>,
    /// so the layout can be felt with the real mouse before it is kept. Deliberately not
    /// persisted across sessions — coming back to unsaved geometry already live would be
    /// a trap, and the mode costs one click.
    /// </summary>
    public bool LiveUpdate
    {
        get => _liveUpdate;
        set => this.RaiseAndSetIfChanged(ref _liveUpdate,value);
    }
   bool _liveUpdate;

    /// <summary>Whether there is a daemon to preview into, and a layout worth previewing.</summary>
    public bool CanLiveUpdate => _canLiveUpdate.Value;
    readonly ObservableAsPropertyHelper<bool> _canLiveUpdate;

    /// <summary>The apply button delivers on click.</summary>
    public ReactiveCommand<Unit, Unit> ManualUpdateCommand { get; }

    /// <summary>The apply button also delivers continuously, as edits are made.</summary>
    public ReactiveCommand<Unit, Unit> LiveUpdateCommand { get; }

    readonly LiveLayoutUpdater _live;
    readonly ILiveTicker _liveTimer;

    /// <summary>
    /// Last Load outcome reported by the daemon ("3 zones (3 main), virtual" / a failure
    /// message). This is the only feedback a Load-without-Run produces — the virtual
    /// layout badge displays it as the simulation status.
    /// </summary>
    public string DaemonLayoutInfo => _daemonLayoutInfo.Value;
    readonly ObservableAsPropertyHelper<string> _daemonLayoutInfo;

    /// <summary>The layout the agent says it holds, or null while it has said nothing.</summary>
    public string? AgentLayoutId
    {
        get => _agentLayoutId;
        private set => this.RaiseAndSetIfChanged(ref _agentLayoutId, value);
    }
    string? _agentLayoutId;

    /// <summary>
    /// The agent holds a different layout from the one shown here. A foreign layout is
    /// excluded: it is not meant to be the agent's, and saying so would be noise.
    /// </summary>
    public bool AgentLayoutMismatch => _agentLayoutMismatch.Value;
    readonly ObservableAsPropertyHelper<bool> _agentLayoutMismatch;

    /// <summary>Ask the agent to rebuild its layout from the displays it can see.</summary>
    public ICommand RefreshCommand { get; }

    public bool IsVirtualLayout => _isVirtualLayout.Value;
    readonly ObservableAsPropertyHelper<bool> _isVirtualLayout;

    public string VirtualLayoutOrigin => _virtualLayoutOrigin.Value;
    readonly ObservableAsPropertyHelper<string> _virtualLayoutOrigin;

    public string StartTooltip => _startTooltip.Value;
    readonly ObservableAsPropertyHelper<string> _startTooltip;

    public string StartTooltipDetail => _startTooltipDetail.Value;
    readonly ObservableAsPropertyHelper<string> _startTooltipDetail;

    public ReactiveCommand<Unit, Unit> BackToLocalLayoutCommand { get; }

   public bool Saved
   {
      get => _saved;
      set => this.RaiseAndSetIfChanged(ref _saved, value);
   }
   bool _saved;
}

/// <summary>The live pump's clock, started and stopped with the preview switch.</summary>
internal interface ILiveTicker
{
    void Start();
    void Stop();
}

/// <summary>
/// The production clock: a <see cref="DispatcherTimer"/>, built and driven on the UI thread.
/// While running it is rooted by the dispatcher — one more reason stopping it belongs to the
/// view model's disposal.
/// </summary>
sealed class DispatcherLiveTicker : ILiveTicker
{
    readonly DispatcherTimer _timer;

    public DispatcherLiveTicker(Func<Task> tick)
    {
        _timer = new DispatcherTimer { Interval = LiveLayoutUpdater.Interval };
        _timer.Tick += async (_, _) => await tick();
    }

    public void Start() => _timer.Start();
    public void Stop() => _timer.Stop();
}
