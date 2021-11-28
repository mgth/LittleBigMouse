/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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

using System.Runtime.Serialization;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Input;
using HLab.DependencyInjection.Annotations;
using HLab.Mvvm;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.ScreenConfig;
using Newtonsoft.Json;

namespace LittleBigMouse.Plugin.Location.Plugins.Location
{
    using H = H<LocationControlViewModel>;

    class LocationControlViewModel : ViewModel<ScreenConfig.ScreenConfig>
    {
        private readonly IMonitorsService _monitorsService;

        [Import]
        private readonly ILittleBigMouseClientService _service;



        public LocationControlViewModel()
        {
            H.Initialize(this);
        }



        [TriggerOn(nameof(Model))]
        public ScreenConfig.ScreenConfig Config => Model;


        [DataContract]
        private class JsonExport
        {
            [DataMember]
            public ScreenConfig.ScreenConfig Config { get; set; }
            [DataMember]
            public ObservableCollectionSafe<Monitor> Monitors { get; set; }
        }

        public ICommand CopyCommand { get; } = H.Command( c =>c
        .Action(
            e =>
            {
                var export = new JsonExport
                {
                    Config = e.Model,
                    Monitors = e._monitorsService.Monitors
                };
                var json = JsonConvert.SerializeObject(export, Formatting.Indented, new JsonSerializerSettings{});
                Clipboard.SetText(json);
            })
       );


        
        public ICommand SaveCommand { get; } = H.Command(
            c => c
                .CanExecute(e => e.Config.Saved == false)
                .Action(e => { e.Config.Save(); })
                .On(e => e.Config.Saved)
                .CheckCanExecute()               
                );



        public ICommand UndoCommand { get; } = H.Command(c => c
            .CanExecute(e => e.Config.Saved == false)
            .Action(e => { e.Config.Load(); })
            .On(e => e.Config.Saved)
            .CheckCanExecute()
        );

        public ICommand StartCommand { get; } = H.Command(c => c
            .CanExecute(e => !(e.Running && (e.Config?.Saved??true)))

                .Action( e => {
                        e.Config.Enabled = true;

                        if (!e.Config.Saved)
                            e.Config.Save();

                        e._service.Start();

                        //e.Client_StateChanged();
                    }
                    )
                .On(e => e.Config.Saved)
                .On(e => e.Running)
                .NotNull(e => e.Config)
                .CheckCanExecute()
        );


        public ICommand StopCommand { get; } = H.Command(c => c
            .CanExecute(e => e.Running)    
            .On(e => e.Running).CheckCanExecute()
            .Action( e => e._service.Stop())
        );

        private readonly IProperty<bool> _running = H.Property< bool>();
        public bool Running {
            get => _running.Get();
            private set => _running.Set(value); }

        private readonly IProperty<bool> _liveUpdate = H.Property< bool>();
        public bool LiveUpdate
        {
            get => _liveUpdate.Get();
            set => _liveUpdate.Set(value);
        }


        [TriggerOn(nameof(LiveUpdate))]
        [TriggerOn(nameof(Model),"Saved")]
        private void DoLiveUpdate()
        {
            if (LiveUpdate && !Config.Saved)
            {
                StartCommand.Execute(null);
            }
        }

        [Import(InjectLocation.AfterConstructor)]
        public LocationControlViewModel(IMonitorsService monitorsService)
        {
            _monitorsService = monitorsService;
            _service.StateChanged += s =>
            {
                switch (s)
                {
                    case "running":
                        Running = true;
                        break;
                    case "stopped":
                        Running = false;
                        break;
                }
            };
            H.Initialize(this);
            _service.Running();
            
        }
    }
}
