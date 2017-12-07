/*
  LittleBigMouse.Control.Core
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Control.Core.

    LittleBigMouse.Control.Core is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Control.Core is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.ComponentModel;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using Hlab.Mvvm;
using Hlab.Mvvm.Commands;
using Hlab.Notify;
using LbmScreenConfig;

namespace LittleBigMouse.Control.Core
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
