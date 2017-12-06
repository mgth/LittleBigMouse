using System;
using System.ComponentModel;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using Erp.Base;
using Erp.Base.Commands;
using Erp.Notify;
using LbmScreenConfig;
using LittleBigMouse_Control;
using Erp.Mvvm;

namespace LittleBigMouse.ControlCore
{
    public class MainViewModel : ViewModel
    {
        public MainViewModel()
        {
            this.Subscribe();
        }

        public ScreenConfig Config
        {
            get => this.Get<ScreenConfig>();
            set => this.Set(value);
        }

        public ViewModeContext Context => this.Get(() => MvvmService.D.MainViewModeContext.AddCreator<ScreenFrameViewModel>(vm => vm.Presenter = Presenter as MultiScreensViewModel));


        public INotifyPropertyChanged Control
        {
            get => this.Get<INotifyPropertyChanged>(); set => this.Set(value);
        }

        public IPresenterViewModel Presenter
        {
            get => this.Get<IPresenterViewModel>();
            set
            {
                if (this.Set(value))
                {
                    Presenter.MainViewModel = this;
                }
            }
        }

        public ModelCommand CloseCommand => this.GetCommand(
            () =>
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

            },
            () => true
        );

        public ModelCommand MaximizeCommand => this.GetCommand(
            () =>
            {
                var w = Application.Current.MainWindow;
                if (w != null)
                {
                    if (w.WindowState == WindowState.Normal)
                        w.WindowState = WindowState.Maximized;
                    else
                    {
                        w.WindowState = WindowState.Normal;
                    }
                }
            },
            () => true
        );

        public void UnMaximize()
        {
            var w = Application.Current.MainWindow;
            if (w != null)
                w.WindowState = WindowState.Normal;
        }

        public StackPanel ButtonPanel { get; } = new StackPanel
        {
            Orientation = Orientation.Horizontal,
        };
       

        public void AddButton(string caption, Action activate, Action deactivate)
        {
            var tb = new ToggleButton
            {
                //Style = (Style)MainView.Resources["ButtonStyle"],
                Content = caption,
            };

            tb.Checked += (sender, args) =>
            {
                foreach (var other in ButtonPanel.Children.OfType<ToggleButton>().Where(e => !ReferenceEquals(e,tb)))
                {
                    other.IsChecked = false;
                }

                activate();
            };

            tb.Unchecked += (sender, args) =>
            {
                deactivate();
            };

            ButtonPanel.Children.Add(tb);
        }

    }
}
