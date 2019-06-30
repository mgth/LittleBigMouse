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
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Net.Http;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using HLab.DependencyInjection.Annotations;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Wpf.Icons;
using HLab.Notify.PropertyChanged;
using LittleBigMouse.ScreenConfigs;

namespace LittleBigMouse.Control.Core
{
    public class MainViewModel : ViewModel<MainViewModel>, IMvvmContextProvider
    {
        [Import]
        public MainViewModel(IIconService iconService)
        {
            IconService = iconService;
            Initialize();
        }

        public IIconService IconService { get; }

        private readonly IProperty<ScreenConfig> _config = H.Property<ScreenConfig>();
        public ScreenConfig Config
        {
            get => _config.Get();
            set => _config.Set(value);
        }


        public INotifyPropertyChanged Control
        {
            get => _control.Get();
            set => _control.Set(value);
        }
        private readonly IProperty<INotifyPropertyChanged> _control = H.Property<INotifyPropertyChanged>(nameof(Control));

        public IPresenterViewModel Presenter
        {
            get => _presenter.Get();
            set
            {
                if (_presenter.Set(value))
                {
                    Presenter.MainViewModel = this;
                }
            }
        }
        private readonly IProperty<IPresenterViewModel> _presenter = H.Property<IPresenterViewModel>(nameof(Presenter));


        public ICommand CloseCommand => _closeCommand.Get();
        private readonly IProperty<ICommand> _closeCommand 
            = H.Property<ICommand>(nameof(CloseCommand), c => c
            .Command( 
                e => e.Close(),
                e => true
            )
        );

        private void Close()
        {
            if (Config.Saved)
            {
                Application.Current.Shutdown();
                return;
            }

            MessageBoxResult result = MessageBox.Show("Save your changes before exiting ?", "Confirmation",
                MessageBoxButton.YesNoCancel, MessageBoxImage.Question);
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


        static async void GraphQl(string q)
        {
            // ... Target page.
            string page = "https://api.github.com/graphql";

            // ... Use HttpClient.
            using (HttpClient client = new HttpClient())
            //using (HttpResponseMessage response = await client.GetAsync(page))
            //using (HttpContent content = response.Content)
            {
                client.BaseAddress = new Uri(page);
                var content = new FormUrlEncodedContent(new []{new KeyValuePair<string, string>(q,q)});
                // ... Read the string.
                var response = client.PostAsync("",content).Result;
            }
        }

        private readonly IProperty<WindowState> _windowState = H.Property<WindowState>(nameof(WindowState));
        public WindowState WindowState
        {
            get => _windowState.Get();
            set => _windowState.Set(value);
        }

        public ICommand MaximizeCommand => _maximizeCommand.Get();
        private readonly IProperty<ICommand> _maximizeCommand = H.Property<ICommand>(nameof(MaximizeCommand),c=>c
            .Set(e => e.CommandService.Get(
                    () =>
                    {
                        if (e.WindowState != WindowState.Normal)
                            e.WindowState = WindowState.Maximized;
                        else
                        {
                            e.WindowState = WindowState.Normal;
                        }
                    },
                    () => true
                    )
            )
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
       

        public void AddButton(object content, string toolTip, Action activate, Action deactivate)
        {
            var tb = new ToggleButton
            {
                ToolTip = toolTip,
                Height = 40,
                Width = 40,
                //Background = new SolidColorBrush(Colors.Black),
                Margin = new Thickness(5),
                Padding = new Thickness(5),
                //BorderBrush = new SolidColorBrush(Colors.Black),
                //Style = (Style)MainView.Resources["ButtonStyle"],
                Content = content,
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

        public void ConfigureMvvmContext(IMvvmContext ctx)
        {
            ctx.AddCreator<ScreenFrameViewModel>(vm => vm.Presenter = Presenter as MultiScreensViewModel);
        }
    }
}
