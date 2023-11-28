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
using System.Collections.Generic;
using System.Linq;
using System.Reactive;
using System.Reactive.Linq;
using System.Runtime.Serialization;
using System.Threading.Tasks;
using Avalonia;
using Avalonia.Threading;
using DynamicData;
using HLab.Base.Avalonia.Extensions;
using HLab.Mvvm.ReactiveUI;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Persistency;
using LittleBigMouse.Zoning;
using Newtonsoft.Json;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Controls;

public class ListItem
{
    public ListItem(string id, string caption, string description)
    {
        Id = id;
        Caption = caption;
        Description = description;
    }

    public string Id { get; }
    public string Caption { get; }
    public string Description { get; }
}

public class LocationControlViewModel : ViewModel<MonitorsLayout>
{
    readonly IMonitorsSet _monitorsService;
    readonly IMainService _mainService;

    readonly ILittleBigMouseClientService _service;

    //public DisplayLayout.MonitorsLayout Layout => _layout.Get();
    //private readonly IProperty<DisplayLayout.MonitorsLayout> _layout = H.BindProperty(e => e.Model);

    public LocationControlViewModel(ILittleBigMouseClientService service,IMainService main, IMonitorsSet monitorsService)
    {
        _service = service;
        _mainService = main;

        _monitorsService = monitorsService;

        _selectedAlgorithm = this.WhenAnyValue(e => e.Model.Algorithm)
            .Select(a => AlgorithmList.Find(e => e.Id == a)).ToProperty(this,nameof(SelectedAlgorithm));

        _selectedPriority = this.WhenAnyValue(e => e.Model.Priority)
            .Select(a => PriorityList.Find(e => e.Id == a)).ToProperty(this,nameof(SelectedPriority));

        CopyCommand = ReactiveCommand.CreateFromTask(CopyAsync);

        SaveCommand = ReactiveCommand.CreateFromTask(
            SaveAsync, 
            this.WhenAnyValue(e => e.Model.Saved, 
            selector: saved  => !saved
            ).ObserveOn(RxApp.MainThreadScheduler));

        UndoCommand = ReactiveCommand.CreateFromTask(
            LoadAsync,
            this.WhenAnyValue(e => e.Model.Saved,selector: s => !s)
            .ObserveOn(RxApp.MainThreadScheduler));

        StartCommand = ReactiveCommand.CreateFromTask(
            StartAsync,
            this.
                WhenAnyValue(
                e => e.Running,
                e => e.Model.Saved,
                (running, saved) => !(running && saved)
                ).ObserveOn(RxApp.MainThreadScheduler));

        StopCommand = ReactiveCommand.CreateFromTask(
            StopAsync
            ,
            this.WhenAnyValue(e => e.Running)
        );

        this.WhenAnyValue(
            e => e.LiveUpdate,
            e => e.Model.Saved
            ).Subscribe(e => DoLiveUpdate());


        this.WhenAnyValue(e => e.Model.Saved)
            .Do(e =>
            {
                Saved = e;
            });

        service.DaemonEventReceived += StateChanged;
        StateChanged(this,new LittleBigMouseServiceEventArgs(service.State,""));
    }

    void StateChanged(object? sender, LittleBigMouseServiceEventArgs e)
    {
        Dispatcher.UIThread.Invoke(new Action(() => 
                Running = e.Event switch
                {
                    LittleBigMouseEvent.Running => true,
                    LittleBigMouseEvent.Stopped => false,
                    LittleBigMouseEvent.Dead => false,
                    _ => Running
                }
            ));
    }

    protected override MonitorsLayout? OnModelChanging(MonitorsLayout? oldModel, MonitorsLayout? newModel)
    {
        if (newModel is { } model)
        {
            model.PhysicalMonitors.AsObservableChangeSet()
                .ObserveOn(RxApp.MainThreadScheduler)
                .WhenValueChanged(e => e.Saved)
                .Do(e =>
                {
                    if (!e) Saved = false;
                }).Subscribe().DisposeWith(this);

            model.PhysicalMonitors.AsObservableChangeSet()
                .ObserveOn(RxApp.MainThreadScheduler)
                .WhenValueChanged(e => e.Model.Saved)
                .Do(e =>
                {
                    if (!e) Saved = false;
                }).Subscribe().DisposeWith(this);

            model.PhysicalSources.AsObservableChangeSet()
                .ObserveOn(RxApp.MainThreadScheduler)
                .WhenValueChanged(e => e.Saved)
                .Do(e =>
                {
                    if (!e) Saved = false;
                }).Subscribe().DisposeWith(this);

        }

        return base.OnModelChanging(oldModel, newModel);
    }

    [DataContract]
    class JsonExport
    {
        [DataMember]
        public MonitorsLayout? Layout { get; init; }
        [DataMember]
        public List<MonitorDevice>? Monitors { get; init; }
        [DataMember]
        public ZonesLayout? Zones { get; init; }
    }

    async Task CopyAsync()
    {
        if(Model == null) return;

           var export = new JsonExport
           {
               Layout = Model,
               Monitors = _monitorsService.Monitors.ToList(),
               Zones = Model?.ComputeZones()
           };
           var json = JsonConvert.SerializeObject(export, Formatting.Indented, new JsonSerializerSettings
           {
               ReferenceLoopHandling = ReferenceLoopHandling.Ignore
           });

           try
           {
               var clipboard = Application.Current?.GetTopLevel()?.Clipboard;
               if (clipboard != null)
               {
                   await clipboard.SetTextAsync(json);
               }
           }
           catch (Exception ex)
           {

           }

    }

    public ReactiveCommand<Unit, Unit> CopyCommand { get; }
    public ReactiveCommand<Unit, Unit> SaveCommand { get; }
    public ReactiveCommand<Unit, Unit> UndoCommand { get; }
    public ReactiveCommand<Unit, Unit> StartCommand { get; }
    public ReactiveCommand<Unit, Unit> StopCommand { get; }

    async Task StartAsync()
    {
        if(Model == null) return;

        Model.Enabled = true;

        if (!Model.Saved)
            await SaveAsync();
        else
        {
            await Task.Run(() => Model.SaveEnabled());
        }

        await _service.StartAsync(_mainService.MonitorsLayout.ComputeZones());
    }

    async Task StopAsync()
    {
        if(Model == null) return;

        Model.Enabled = false; 
        await Task.Run(() => Model.SaveEnabled());
        await _service.StopAsync();
    }

    Task SaveAsync() =>
        Task.Run(() =>
        {
            if (!(Model?.Saved??true))
                Model.Save();
        });

    Task LoadAsync() =>
        Task.Run(() =>
        {
            if (!(Model?.Saved??true))
                Model.Load();
        });


    public ListItem? SelectedAlgorithm
    {
        get => _selectedAlgorithm.Value;
        set
        {
            if(Model == null) return;
            Model.Algorithm = value?.Id ?? "";
        }
    }
    readonly ObservableAsPropertyHelper<ListItem?> _selectedAlgorithm;

    public ListItem? SelectedPriority
    {
        get => _selectedPriority.Value;
        set
        {
            if(Model == null) return;
            Model.Priority = value?.Id ?? "";
        }
    }
    readonly ObservableAsPropertyHelper<ListItem?> _selectedPriority;


    public bool Running
    {
        get => _running;
        private set => this.RaiseAndSetIfChanged(ref _running,value);
    }
    bool _running;

    public bool LiveUpdate
    {
        get => _liveUpdate;
        set => this.RaiseAndSetIfChanged(ref _liveUpdate,value);
    }
    bool _liveUpdate;

    public List<ListItem> AlgorithmList { get; } = new()
    {
        new ("Strait","Strait","Simple and highly CPU-efficient transition."),
        new ("Cross","Corner crossing","In direction-friendly manner, allows traversal through corners."),
    };

    public List<ListItem> PriorityList { get; } = new()
    {
        new ("Idle","Idle",""),
        new ("Below","Below",""),
        new ("Normal","Normal",""),
        new ("Above","Above",""),
        new ("High","High",""),
        new ("Realtime","Realtime",""),

    };
    void DoLiveUpdate()
    {
        if (LiveUpdate && !Saved)
        {
            StartCommand.Execute();
        }
    }

}
