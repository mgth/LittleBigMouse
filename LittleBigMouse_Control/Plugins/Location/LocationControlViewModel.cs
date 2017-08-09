using System;
using System.Collections.Generic;
using System.Linq;
using System.Windows.Input;
using Erp.Notify;
using LbmScreenConfig;
using LittleBigMouse_Control.PluginLocation;
using LittleBigMouse_Control.Rulers;

namespace LittleBigMouse_Control.Plugins.Location
{
    class LocationControlViewModel : ViewModel
    {
        public override Type ViewType => typeof (LocationControlView);

        public ScreenConfig Config
        {
            get => this.Get<ScreenConfig>(); set
            {
                if (this.Set(value))
                {
                    SaveCommand = new SaveCommand(Config);
                    StartCommand = new StartCommand(Config);
                    StopCommand = new StopCommand(Config);
                    LittleBigMouseClient.Client.StateChanged += Client_StateChanged;

                    View.Unloaded += View_Unloaded;
                }
            }
        }

        private void View_Unloaded(object sender, System.Windows.RoutedEventArgs e)
        {
            ShowRulers = false;
        }

        private void Client_StateChanged()
        {
            Running = LittleBigMouseClient.Client.Running();
        }

        public String StatStopLabel => LittleBigMouseClient.Client.Running()?"Stop":"Start";

        public SaveCommand SaveCommand { get; private set; }
        public StartCommand StartCommand { get; private set; }
        public StopCommand StopCommand { get; private set; }

        public bool ShowRulers { get => this.Get<bool>(); set => this.Set(value); }
        public bool Running { get => this.Get<bool>(); private set => this.Set(value); }

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
        [TriggedOn("Config.Saved")]
        private void DoLiveUpdate()
        {
            if (LiveUpdate && !Config.Saved)
            {
                StartCommand.Execute(null);
            }
        }

        private readonly List<Ruler> _rulers = new List<Ruler>();
        private void AddRuler(RulerViewModel.RulerSide side)
        {
            if (Config.Selected == null) return;

            foreach (var sz in Config.AllScreens.Select(s => new RulerViewModel(Config.Selected, s, side)))
            {
                var r = new Ruler {DataContext = sz}; 
                _rulers.Add(r);
            }
        }

        private Tester _tester;
        private void ShowTester()
        {
            if(_tester == null)
                _tester = new Tester {DataContext = new TesterViewModel()};

            _tester.Show();
        }




        [TriggedOn(nameof(ShowRulers))]
        [TriggedOn("Config.Selected")]
        void UpdateRulers()
        {
            //if(ShowRulers)
            //    ShowTester();
            //else
            //{
            //    _tester?.Close();
            //    _tester = null;
            //}

            //return;

            foreach (var sz in _rulers)
            {
                if(!sz.IsClosing)
                    sz.Close();
            }
            _rulers.Clear();

            if (!ShowRulers) return;

            AddRuler(RulerViewModel.RulerSide.Left);
            AddRuler(RulerViewModel.RulerSide.Right);
            AddRuler(RulerViewModel.RulerSide.Top);
            AddRuler(RulerViewModel.RulerSide.Bottom);

            foreach (var ruler in _rulers) ruler.ViewModel.Enabled = true;
        }
    }

    class SaveCommand : ICommand
    {
        private readonly ScreenConfig _config;

        public SaveCommand(ScreenConfig config)
        {
            _config = config;
            config.PropertyChanged += Config_PropertyChanged;
        }

        private void Config_PropertyChanged(object sender, System.ComponentModel.PropertyChangedEventArgs e)
        {
            if (e.PropertyName=="Saved")
                CanExecuteChanged?.Invoke(this,new EventArgs());
        }

        #region ICommand Members

        public bool CanExecute(object parameter)
        {
            return !_config.Saved;
        }

        public event EventHandler CanExecuteChanged;
        public void Execute(object parameter)
        {
            _config.Save();
        }
        #endregion
    }

    class StartCommand : ICommand
    {
        private readonly ScreenConfig _config;

        public StartCommand(ScreenConfig config)
        {
            _config = config;
            LittleBigMouseClient.Client.StateChanged += Client_StateChanged;
        }

        private void Client_StateChanged()
        {
            //CanExecuteChanged?.Invoke(this,null);
        }

        #region ICommand Members

        public bool CanExecute(object parameter)
        {
            return true;
            //return !LittleBigMouseClient.Client.Running();
        }

        public event EventHandler CanExecuteChanged;
        public void Execute(object parameter)
        {
            //if (!LittleBigMouseClient.Client.Running())
            //    LittleBigMouseClient.Client.Stop();
            //else
            if (!_config.Saved) _config.Save();

            if (!LittleBigMouseClient.Client.Running())
                LittleBigMouseClient.Client.Start();
            else
            {
                _config.Save();
                LittleBigMouseClient.Client.LoadConfig();
            }
        }
        #endregion
    }

    class StopCommand : ICommand
    {
        private readonly ScreenConfig _config;

        public StopCommand(ScreenConfig config)
        {
            _config = config;
            LittleBigMouseClient.Client.StateChanged += Client_StateChanged;
        }

        private void Client_StateChanged()
        {
            //CanExecuteChanged?.Invoke(this,null);
        }

        #region ICommand Members

        public bool CanExecute(object parameter)
        {
            return true;
            //return !LittleBigMouseClient.Client.Running();
        }

        public event EventHandler CanExecuteChanged;
        public void Execute(object parameter)
        {
            //if (!LittleBigMouseClient.Client.Running())
            //    LittleBigMouseClient.Client.Stop();
            //else
            LittleBigMouseClient.Client.Stop();
        }
        #endregion
    }
}
