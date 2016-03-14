using System;
using System.Collections.Generic;
using System.Linq;
using System.Windows.Input;
using LbmScreenConfig;
using NotifyChange;

namespace LittleBigMouse_Control.PluginLocation
{
    class LocationControlViewModel : ViewModel
    {
        public override Type ViewType => typeof (LocationControlView);

        public ScreenConfig Config
        {
            get { return _config; }
            set
            {
                if (SetAndWatch(ref _config, value))
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
        private ScreenConfig _config;

        private bool _showRulers;
        public bool ShowRulers
        {
            get { return _showRulers; }
            set { SetProperty(ref _showRulers,value); }
        }

        private bool _running;
        public bool Running
        {
            get { return _running; }
            private set { SetProperty(ref _running, value); }
        }
        private bool _liveUpdate;
        public bool LiveUpdate
        {
            get { return _liveUpdate; }
            set { SetProperty(ref _liveUpdate, value); }
        }
        [DependsOn(nameof(LiveUpdate), "Config.Saved")]
        private void DoLiveUpdate()
        {
            if (LiveUpdate && !Config.Saved)
            {
                StartCommand.Execute(null);
            }
        }

        private readonly List<Ruler> _rulers = new List<Ruler>();
        private void AddRuler(RulerSide side)
        {
            if (Config.Selected == null) return;

            foreach (var sz in Config.AllScreens.Select(s => new Ruler(Config.Selected, s, side)))
            {
                _rulers.Add(sz);
            }
        }

        [DependsOn("ShowRulers", "Config.Selected")]
        void UpdateRulers()
        {
            foreach (Ruler sz in _rulers)
            {
                sz.Close();
            }
            _rulers.Clear();

            if (_showRulers)
            {
                AddRuler(RulerSide.Left);
                AddRuler(RulerSide.Right);
                AddRuler(RulerSide.Top);
                AddRuler(RulerSide.Bottom);

                foreach (Ruler ruler in _rulers) ruler.Enabled = true;
            }
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
