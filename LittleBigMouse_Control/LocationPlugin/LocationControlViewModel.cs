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
                if (SetProperty(ref _config, value))
                {
                    SaveCommand = new SaveCommand(Config);
                }
            }
        }

        public ICommand SaveCommand;
        private ScreenConfig _config;
    }

    class SaveCommand : ICommand
    {
        private ScreenConfig _config;

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
            return _config.Saved;
        }

        public event EventHandler CanExecuteChanged;
        public void Execute(object parameter)
        {
            _config.Save();
        }
        #endregion
    }
}
