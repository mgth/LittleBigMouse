using System;
using System.Collections.ObjectModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Data;
using System.Windows.Input;
using Erp.Notify;
using LbmScreenConfig;
using LittleBigMouse_Control.Plugins;

namespace LittleBigMouse_Control
{
    internal class MainViewModel : ViewModel
    {
        //public override Type ViewType => typeof (MainView);

        public MainViewModel()
        {
            CloseCommand = new CloseCommand(this);
            MaximizeCommand = new MaximizeCommand(this);

            Plugins.CollectionChanged += Plugins_CollectionChanged;
        }

        private void Plugins_CollectionChanged(object sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
        {
            if (e.NewItems == null) return;
            foreach (Plugin plugin in e.NewItems)
            {
                plugin.MainViewModel = this;
                plugin.Init();
            }
        }

        public readonly ObservableCollection<Plugin> Plugins = new ObservableCollection<Plugin>();

        //private GetScreenControlViewModelDelegate _getScreenControlView;

        public ScreenConfig Config
        {
            get => this.Get<ScreenConfig>(); set => this.Set(value);
        }


        public ViewModel Control
        {
            get => this.Get<ViewModel>(); set => this.Set(value);
        }

        public PresenterViewModel Presenter
        {
            get => this.Get<PresenterViewModel>(); set
            {
                if (this.Set(value))
                {
                    Presenter.MainViewModel = this;
                }
            }
        }

        public CloseCommand CloseCommand { get; }
        public MaximizeCommand MaximizeCommand { get; }

        public void Close()
        {
            if (Config.Saved)
            {
                Application.Current.Shutdown();
                return;
            }

            MessageBoxResult result = MessageBox.Show("Save your changes before exiting ?", "Confirmation", MessageBoxButton.YesNoCancel, MessageBoxImage.Question);
            if (result == MessageBoxResult.Yes)
            {
                Config.Save();
                Application.Current.Shutdown();
            }

            if (result == MessageBoxResult.No)
            {
                Application.Current.Shutdown();
            }
        }

        public void Maximize()
        {
            Window w = (View as Window);
            if(w!=null)
            if (w.WindowState==WindowState.Normal)
                w.WindowState = WindowState.Maximized;
            else
            {
                w.WindowState = WindowState.Normal;
            }
        }
        public void UnMaximize()
        {
            Window w = (View as Window);
            if (w != null)
                w.WindowState = WindowState.Normal;
        }

        public StackPanel ButtonPanel { get; } = new StackPanel
        {
            Orientation = Orientation.Horizontal,
        };
       

        public void AddButton(IPluginButton plugin)
        {
            ToggleButton tb = new ToggleButton
            {
                //Style = (Style)MainView.Resources["ButtonStyle"],
                Content = plugin.Caption,
                DataContext = plugin,
            };

            Binding binding = new Binding
            {
                Mode = BindingMode.TwoWay,
                Path = new PropertyPath("IsActivated"),
            };

            tb.SetBinding(ToggleButton.IsCheckedProperty, binding);

            tb.Checked += Tb_Checked;

            ButtonPanel.Children.Add(tb);
        }

        private void Tb_Checked(object sender, RoutedEventArgs e)
        {
            foreach (ToggleButton tb in ButtonPanel.Children)
            {
                if (tb != sender) tb.IsChecked = false;
            }
        }
    }

    class CloseCommand : ICommand
    {
        private readonly MainViewModel _main;

        public CloseCommand(MainViewModel main)
        {
            _main = main;
        }

         #region ICommand Members

        public bool CanExecute(object parameter)
        {
            return true;
        }

        public event EventHandler CanExecuteChanged;
        public void Execute(object parameter)
        {
            _main.Close();
        }
        #endregion
    }

    class MaximizeCommand : ICommand
    {
        private readonly MainViewModel _main;

        public MaximizeCommand(MainViewModel main)
        {
            _main = main;
        }

        #region ICommand Members

        public bool CanExecute(object parameter)
        {
            return true;
        }

        public event EventHandler CanExecuteChanged;
        public void Execute(object parameter)
        {
            _main.Maximize();
        }
        #endregion
    }

}
