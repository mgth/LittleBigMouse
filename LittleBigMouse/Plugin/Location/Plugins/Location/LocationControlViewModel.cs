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

using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Runtime.Serialization;
using System.Windows;
using System.Xml;
using System.Xml.Serialization;
using HLab.Mvvm;
using HLab.Mvvm.Commands;
using HLab.Notify;
using HLab.Windows.Monitors;
using LittleBigMouse.LocationPlugin.Plugins.Location.Rulers;
using LittleBigMouse.ScreenConfigs;
using Newtonsoft.Json;
using Formatting = System.Xml.Formatting;
using Tester = LittleBigMouse.Plugin.Location.Plugins.Location.Rulers.Tester;

namespace LittleBigMouse.Plugin.Location.Plugins.Location
{
    class LocationControlViewModel : IViewModel<ScreenConfig>
    {
        public ScreenConfig Model => this.GetModel();
        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }

        [TriggedOn(nameof(Model))]
        public ScreenConfig Config => this.Get(()=>Model);


        private void Client_StateChanged()
        {
            Running = LittleBigMouseClient.Client.Running();
        }

        private class JsonExport
        {
            [JsonProperty]
            public ScreenConfig Config { get; set; }
            [JsonProperty]
            public ObservableCollection<Monitor> Monitors { get; set; }
        }

        public ModelCommand CopyCommand => this.GetCommand(() =>
            {
                var export = new JsonExport{Config=Model,Monitors = MonitorsService.D.Monitors};
                var json = JsonConvert.SerializeObject(export, Newtonsoft.Json.Formatting.Indented);
                Clipboard.SetText(json);
            },
        ()=>true
        );

        public String StatStopLabel => LittleBigMouseClient.Client.Running()?"Stop":"Start";

        [TriggedOn(nameof(Config),"Saved")]
        public ModelCommand SaveCommand => this.GetCommand(() =>
            {
                Config.Save();
            },
        ()=>Config.Saved == false
        );
        [TriggedOn(nameof(Config), "Saved")]
        public ModelCommand UndoCommand => this.GetCommand(() =>
            {
                Config.Load();
            },
            () => Config.Saved == false
        );

        [TriggedOn(nameof(Running))]
        [TriggedOn(nameof(Config),"Saved")]
        public ModelCommand StartCommand => this.GetCommand(
            () =>
            {
                Config.Enabled = true;

                if (!Config.Saved)
                    Config.Save();

                LittleBigMouseClient.Client.LoadConfig();

                if (!Running)
                    LittleBigMouseClient.Client.Start();

                Client_StateChanged();
            }, 
            () => !(Running && Config.Saved));

        [TriggedOn(nameof(Running))]
        public ModelCommand StopCommand => this.GetCommand(
            () => {
                LittleBigMouseClient.Client.Stop();
                Client_StateChanged();
            },
            () => Running);

        public bool Running {
            get => this.Get<bool>();
            private set => this.Set(value); }

        public bool LiveUpdate { get => this.Get<bool>(); set => this.Set(value); }

        public bool LoadAtStartup
        {
            get => Config.LoadAtStartup; set
            {
                Config.LoadAtStartup = value;
                LittleBigMouseClient.Client.LoadAtStartup(value);
            }
        }


        [TriggedOn(nameof(LiveUpdate))]
        [TriggedOn(nameof(Model),"Saved")]
        private void DoLiveUpdate()
        {
            if (LiveUpdate && !Config.Saved)
            {
                StartCommand.Execute(null);
            }
        }


        public LocationControlViewModel()
        {
            LittleBigMouseClient.Client.StateChanged += Client_StateChanged;
            Client_StateChanged();
            this.SubscribeNotifier();
        }
    }
}
