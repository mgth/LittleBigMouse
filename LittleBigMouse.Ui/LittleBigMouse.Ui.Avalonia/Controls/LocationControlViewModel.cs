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
using System.Reactive;
using System.Reactive.Concurrency;
using System.Reactive.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Tasks;
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

    readonly ILittleBigMouseClientService _service;

    public LocationControlViewModel(ILittleBigMouseClientService service,IMainService main, ISystemMonitorsService monitorsService, ILayoutPersistence persistence)
    {
        _service = service;
        _mainService = main;
        _persistence = persistence;

        _monitorsService = monitorsService;

        // What the daemon says about itself — see DaemonStatusTracker. Built before the commands
        // below, whose canExecute reads Running and Dead through the helpers wired here.
        _status = new DaemonStatusTracker(
            run => Dispatcher.UIThread.Invoke(run),
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
            () => Model?.ComputeZones(),
            (zones, token) => _service.SendLiveAsync(zones, token));

        _liveTimer = new DispatcherTimer { Interval = LiveLayoutUpdater.Interval };
        _liveTimer.Tick += async (_, _) => await _live.TickAsync();

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

        service.DaemonEventReceived += (_, e) => _status.Apply(e);
        _status.Apply(new LittleBigMouseServiceEventArgs(service.State, ""));
    }

    readonly DaemonStatusTracker _status;

    protected override MonitorsLayout? OnModelChanging(MonitorsLayout? oldModel, MonitorsLayout? newModel)
    {
        // The last Load outcome belongs to the previous layout generation. Runs before the
        // field is assigned on the very first model, where there is nothing to forget.
        Dispatcher.UIThread.Post(() => _status?.ForgetLayoutInfo());

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

    async Task StartAsync()
    {
        if(Model == null) return;

        // Virtual layout: "simulate" — hand the zones to the daemon for validation
        // (the client service sends Load without Run). Nothing is persisted, and
        // Enabled stays false so a layout rebuild can never auto-engage a foreign
        // geometry.
        if (Model.IsVirtual)
        {
            await _service.StartAsync(Model.ComputeZones());
            return;
        }

        Model.Options.Enabled = true;

        if (!Model.Saved)
            await SaveAsync();
        else
            await SaveEnabledAsync();

        await _service.StartAsync(_mainService.MonitorsLayout.ComputeZones());
    }

    async Task StopAsync()
    {
        if(Model == null) return;

        // Asking for the engine to stop outranks previewing into it — otherwise the
        // next tick would hook it straight back up.
        LiveUpdate = false;

        Model.Options.Enabled = false;
        await Task.Run(() => _persistence.SaveEnabled(Model));
        await _service.StopAsync();
    }

    Task SaveAsync() =>
        Task.Run(() =>
        {
            if (!(Model?.Saved??true))
                _persistence.Save(Model);
        });

    Task SaveEnabledAsync() =>
        Task.Run(() =>
        {
            if (Model != null) _persistence.SaveEnabled(Model);
        });

    /// <summary>
    /// The panic shortcut interrupted a live preview: throw the experiment away and put
    /// the engine back on the layout the user actually saved. Nothing is written — the
    /// preview never was — so this is a pure reload plus a fresh start.
    /// </summary>
    async Task AbandonPreviewAsync()
    {
        await LoadAsync();
        if (Model is null || Model.IsVirtual) return;
        await _service.StartAsync(Model.ComputeZones());
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
    readonly DispatcherTimer _liveTimer;

    /// <summary>
    /// Last Load outcome reported by the daemon ("3 zones (3 main), virtual" / a failure
    /// message). This is the only feedback a Load-without-Run produces — the virtual
    /// layout badge displays it as the simulation status.
    /// </summary>
    public string DaemonLayoutInfo => _daemonLayoutInfo.Value;
    readonly ObservableAsPropertyHelper<string> _daemonLayoutInfo;

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
