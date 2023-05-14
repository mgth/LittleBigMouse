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

using System.Reactive;
using System.Runtime.Serialization;
using Avalonia;
using Avalonia.Controls;
using HLab.Base.Avalonia.Extensions;
using HLab.Mvvm.ReactiveUI;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Zoning;
using Newtonsoft.Json;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Layout.Avalonia.LocationPlugin;

internal class LocationControlViewModel : ViewModel<MonitorsLayout>
{
    readonly IMonitorsSet _monitorsService;

    readonly ILittleBigMouseClientService _service;

    //public DisplayLayout.MonitorsLayout Layout => _layout.Get();
    //private readonly IProperty<DisplayLayout.MonitorsLayout> _layout = H.BindProperty(e => e.Model);

    public LocationControlViewModel(ILittleBigMouseClientService service, IMonitorsSet monitorsService)
    {
        _service = service;
        _monitorsService = monitorsService;

        CopyCommand = ReactiveCommand.CreateFromTask(CopyAsync);

        // TODO
        //SaveCommand = ReactiveCommand.Create(
        //    Model.Save, 
        //    this.WhenAnyValue(e => e.Model.Saved,selector: (saved) => !saved)
        //    );

        //UndoCommand = ReactiveCommand.Create(Model.Load);

        StartCommand = ReactiveCommand.CreateFromTask(
            StartAsync,
            this.WhenAnyValue(
                e => e.Running,
                e => e.Model.Saved,
                (running, saved) => !(running && saved)
                ));

        StopCommand = ReactiveCommand.CreateFromTask(
            _service.StopAsync,
            this.WhenAnyValue(e => e.Running)
        );

        this.WhenAnyValue(
            e => e.LiveUpdate,
            e => e.Model.Saved
            ).Subscribe(e => DoLiveUpdate());

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

    public ReactiveCommand<Unit, bool> SaveCommand { get; }

    public ReactiveCommand<Unit, Unit> UndoCommand { get; }

    public ReactiveCommand<Unit, Unit> StartCommand { get; }
    public ReactiveCommand<Unit, Unit> StopCommand { get; }

    async Task StartAsync()
    {
        Model.Enabled = true;

        // TODO
        //if (!Model.Saved)
        //    Model.Save();

        await _service.StartAsync(Model.ComputeZones());
    }

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


    void DoLiveUpdate()
    {
        if (LiveUpdate && !Model.Saved)
        {
            StartCommand.Execute();
        }
    }

}
