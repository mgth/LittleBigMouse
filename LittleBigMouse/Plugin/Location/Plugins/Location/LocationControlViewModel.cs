using System;
using System.ComponentModel;
using LbmScreenConfig;
using Hlab.Mvvm;
using Hlab.Mvvm.Commands;
using Hlab.Notify;
using LittleBigMouse.LocationPlugin.Plugins.Location.Rulers;
using Tester = LittleBigMouse.LocationPlugin.Plugins.Location.Rulers.Tester;

namespace LittleBigMouse.LocationPlugin.Plugins.Location
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

        public String StatStopLabel => LittleBigMouseClient.Client.Running()?"Stop":"Start";

        [TriggedOn(nameof(Config),"Saved")]
        public ModelCommand SaveCommand => this.GetCommand(() =>
            {
                Config.Save();
            },
        ()=>Config.Saved == false
        );

        [TriggedOn(nameof(Running))]
        [TriggedOn(nameof(Config),"Saved")]
        public ModelCommand StartCommand => this.GetCommand(
            () =>
            {
                Config.Enabled = true;

                if (!Config.Saved) Config.Save();

                if (!Running)
                    LittleBigMouseClient.Client.Start();
                else
                {
                    Config.Save();
                    LittleBigMouseClient.Client.LoadConfig();
                }
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


        private Tester _tester;

        public LocationControlViewModel()
        {
            LittleBigMouseClient.Client.StateChanged += Client_StateChanged;
            Client_StateChanged();
            this.Subscribe();
        }


        private void ShowTester()
        {
            if(_tester == null)
                _tester = new Tester {DataContext = new TesterViewModel()};

            _tester.Show();
        }





    }

 
    
}
