/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Location.

    LittleBigMouse.Plugin.Location is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Location is distributed in the hope that it will be useful,
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

        _startTooltip = this.WhenAnyValue(e => e.Model)
            .Select(m => m?.IsVirtual == true
                ? "Simulate: send this layout to the daemon for validation (input hook stays off)"
                : "Apply/Start")
            .ToProperty(this, e => e.StartTooltip);

        BackToLocalLayoutCommand = ReactiveCommand.Create(
            _mainService.ReloadSystemLayout,
            this.WhenAnyValue(e => e.Model)
                .Select(m => m?.IsVirtual ?? false)
                .ObserveOn(RxSchedulers.MainThreadScheduler));

        this.WhenAnyValue(
            e => e.LiveUpdate,
            e => e.Model.Saved
            ).Subscribe(e => DoLiveUpdate());


        this.UnsavedOn(e => e.Model);

        service.DaemonEventReceived += StateChanged;
        StateChanged(this,new LittleBigMouseServiceEventArgs(service.State,""));
    }

    void StateChanged(object? sender, LittleBigMouseServiceEventArgs e)
    {
        try
        {
            switch (e.Event)
            {
                case LittleBigMouseEvent.Running:
                    Dispatcher.UIThread.Invoke(() =>
                    {
                        Dead = false;
                        Running = true;
                    });
                    break;
                case LittleBigMouseEvent.Stopped:
                    Dispatcher.UIThread.Invoke(() =>
                    {
                        Dead = false;
                        Running = false;
                    });
                    break;
                case LittleBigMouseEvent.Dead:
                    Dispatcher.UIThread.Invoke(() =>
                    {
                        Dead = true;
                        Running = false;
                    });
                    break;
                case LittleBigMouseEvent.Loaded:
                    Dispatcher.UIThread.Invoke(() => DaemonLayoutInfo = e.Payload);
                    break;
                case LittleBigMouseEvent.LoadFailed:
                    Dispatcher.UIThread.Invoke(() =>
                        DaemonLayoutInfo = string.IsNullOrEmpty(e.Payload) ? "load failed" : e.Payload);
                    break;
                case LittleBigMouseEvent.SettingsChanged:
                case LittleBigMouseEvent.DisplayChanged:
                case LittleBigMouseEvent.DesktopChanged:
                case LittleBigMouseEvent.FocusChanged:
                case LittleBigMouseEvent.Paused:
                case LittleBigMouseEvent.Connected:
                    break;
                default:
                    throw new ArgumentOutOfRangeException(nameof(e.Event), e.Event, null);
            }
            //Dispatcher.UIThread.Invoke(new Action(() =>
            //    Running = e.Event switch
            //    {
            //        LittleBigMouseEvent.Running => true,
            //        LittleBigMouseEvent.Stopped => false,
            //        LittleBigMouseEvent.Dead => false,
            //        _ => Running
            //    }
            //));
        }
        catch(TaskCanceledException)
        {
             
        }
    }

    protected override MonitorsLayout? OnModelChanging(MonitorsLayout? oldModel, MonitorsLayout? newModel)
    {
        // The last Load outcome belongs to the previous layout generation.
        Dispatcher.UIThread.Post(() => DaemonLayoutInfo = "");

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

    Task LoadAsync() =>
        Task.Run(() =>
        {
            if (!(Model?.Saved??true))
                _persistence.Load(Model);
        });




    public bool Running
    {
        get => _running;
        private set => this.RaiseAndSetIfChanged(ref _running,value);
    }
    bool _running;

    public bool Dead
    {
        get => _dead;
        private set => this.RaiseAndSetIfChanged(ref _dead,value);
    }
    bool _dead;

    public bool LiveUpdate
    {
        get => _liveUpdate;
        set => this.RaiseAndSetIfChanged(ref _liveUpdate,value);
    }
   bool _liveUpdate;

    /// <summary>
    /// Last Load outcome reported by the daemon ("3 zones (3 main), virtual" / a failure
    /// message). This is the only feedback a Load-without-Run produces — the virtual
    /// layout badge displays it as the simulation status.
    /// </summary>
    public string DaemonLayoutInfo
    {
        get => _daemonLayoutInfo;
        private set => this.RaiseAndSetIfChanged(ref _daemonLayoutInfo, value);
    }
    string _daemonLayoutInfo = "";

    public bool IsVirtualLayout => _isVirtualLayout.Value;
    readonly ObservableAsPropertyHelper<bool> _isVirtualLayout;

    public string VirtualLayoutOrigin => _virtualLayoutOrigin.Value;
    readonly ObservableAsPropertyHelper<string> _virtualLayoutOrigin;

    public string StartTooltip => _startTooltip.Value;
    readonly ObservableAsPropertyHelper<string> _startTooltip;

    public ReactiveCommand<Unit, Unit> BackToLocalLayoutCommand { get; }

   public bool Saved
   {
      get => _saved;
      set => this.RaiseAndSetIfChanged(ref _saved, value);
   }
   bool _saved;


    void DoLiveUpdate()
    {
        if (LiveUpdate && !Saved)
        {
            StartCommand.Execute();
        }
    }

}
