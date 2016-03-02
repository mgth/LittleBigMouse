using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Input;
using LbmScreenConfig;
using NotifyChange;

namespace LittleBigMouse_Control.LocationPlugin
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
                    StartStopCommand = new StartStopCommand();
                }
            }
        }

        public String StatStopLabel => LittleBigMouseClient.Client.Running()?"Stop":"Start";

        public SaveCommand SaveCommand { get; private set; }
        public StartStopCommand StartStopCommand { get; private set; }
        private ScreenConfig _config;
        private bool _showRulers;

        public bool ShowRulers
        {
            get { return _showRulers; }
            set { SetProperty(ref _showRulers,value); }
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

    class StartStopCommand : ICommand
    {
        //private readonly ScreenConfig _config;

        public StartStopCommand(/*ScreenConfig config*/)
        {
            //_config = config;
        }

        #region ICommand Members

        public bool CanExecute(object parameter)
        {
            return true;
        }

        public event EventHandler CanExecuteChanged;
        public void Execute(object parameter)
        {
            //if (!LittleBigMouseClient.Client.Running())
            //    LittleBigMouseClient.Client.Stop();
            //else
                LittleBigMouseClient.Client.Start();
        }
        #endregion
    }
}
