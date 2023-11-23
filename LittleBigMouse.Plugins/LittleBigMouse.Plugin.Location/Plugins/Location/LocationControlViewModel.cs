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
using System.Runtime.Serialization;
using System.Windows;
using System.Windows.Input;

using HLab.Mvvm;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.Monitors;

using LittleBigMouse.DisplayLayout;
using LittleBigMouse.Zoning;

using Newtonsoft.Json;

namespace LittleBigMouse.Plugin.Location.Plugins.Location;

using H = H<LocationControlViewModel>;

class LocationControlViewModel : ViewModel<Layout>
{
    private readonly IMonitorsService _monitorsService;

    private readonly ILittleBigMouseClientService _service;

    public Layout Layout => _layout.Get();
    private readonly IProperty<Layout> _layout = H.BindProperty(e => e.Model);

    [DataContract]
    private class JsonExport
    {
        [DataMember]
        public Layout Layout { get; set; }
        [DataMember]
        public List<MonitorDevice> Monitors { get; set; }
        [DataMember]
        public ZonesLayout Zones { get; set; }
    }

    public ICommand CopyCommand { get; } = H.Command(c => c
   .Action(
       e =>
       {
           var export = new JsonExport
           {
               Layout = e.Model,
               Monitors = e._monitorsService.Monitors.Items.ToList(),
               Zones = e.Model.ComputeZones()
           };
           var json = JsonConvert.SerializeObject(export, Formatting.Indented, new JsonSerializerSettings { 
               ReferenceLoopHandling = ReferenceLoopHandling.Ignore
               });
           try
           {
               Clipboard.SetText(json); 
           }
           catch(Exception ex)
           { }
           
       })
   );

    public ICommand SaveCommand { get; } = H.Command(
        c => c
            .CanExecute(e => e.Layout.Saved == false)
            .Action(e => { e.Layout.Save(); })
            .On(e => e.Layout.Saved)
            .CheckCanExecute()
            );

    public ICommand UndoCommand { get; } = H.Command(c => c
        .CanExecute(e => e.Layout.Saved == false)
        .Action(e => { e.Layout.Load(); })
        .On(e => e.Layout.Saved)
        .CheckCanExecute()
    );

    public ICommand StartCommand { get; } = H.Command(c => c
        .CanExecute(e => !(e.Running && (e.Layout?.Saved ?? true)))

            .Action(e =>
                {
                    e.Layout.Enabled = true;

                    if (!e.Layout.Saved)
                        e.Layout.Save();

                    e._service.Start(e.Layout.ComputeZones());

                }
            )
            .On(e => e.Layout.Saved)
            .On(e => e.Running)
            .NotNull(e => e.Layout)
            .CheckCanExecute()
    );


    public ICommand StopCommand { get; } = H.Command(c => c
        .CanExecute(e => e.Running)
        .On(e => e.Running).CheckCanExecute()
        .Action(e => e._service.Stop())
    );

    private readonly IProperty<bool> _running = H.Property<bool>();
    public bool Running
    {
        get => _running.Get();
        private set => _running.Set(value);
    }

    public bool LiveUpdate
    {
        get => _liveUpdate.Get();
        set => _liveUpdate.Set(value);
    }
    private readonly IProperty<bool> _liveUpdate = H.Property<bool>();


    private ITrigger _onLiveUpdate = H.Trigger(c => c
        .On(e => e.LiveUpdate)
        .On(e =>e.Model.Saved)
        .Do(e => e.DoLiveUpdate())
    );

    private void DoLiveUpdate()
    {
        if (LiveUpdate && !Layout.Saved)
        {
            StartCommand.Execute(null);
        }
    }

    public LocationControlViewModel(ILittleBigMouseClientService service, IMonitorsService monitorsService)
    {
        _service = service;
        _monitorsService = monitorsService;

        H.Initialize(this);
    }
}
