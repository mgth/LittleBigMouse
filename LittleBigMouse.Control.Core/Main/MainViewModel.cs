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

using System.Collections.ObjectModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using HLab.DependencyInjection.Annotations;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Notify.PropertyChanged;

namespace LittleBigMouse.Control.Main
{
    using H = H<MainViewModel>;

    [Export(typeof(MainViewModel)),Singleton]
    public class MainViewModel : ViewModel
    {
        [Import]
        public MainViewModel(IIconService iconService)
        {
            IconService = iconService;
            H.Initialize(this);
        }

        public IIconService IconService { get; }

        public ScreenConfig.ScreenConfig Config
        {
            get => _config.Get();
            set => _config.Set(value);
        }
        private readonly IProperty<ScreenConfig.ScreenConfig> _config = H.Property<ScreenConfig.ScreenConfig>();

        public IPresenterViewModel Presenter
        {
            get => _presenter.Get();
            set => _presenter.Set(value);
        }
        private readonly IProperty<IPresenterViewModel> _presenter = H.Property<IPresenterViewModel>();

        public double VerticalResizerSize => _verticalResizerSize.Get();
        private readonly IProperty<double> _verticalResizerSize = H.Property<double>(c => c.Default(10.0));
        public double HorizontalResizerSize => _horizontalResizerSize.Get();
        private readonly IProperty<double> _horizontalResizerSize = H.Property<double>(c => c.Default(10.0));


        public ICommand CloseCommand { get; } = H.Command(c => c
            .Action(e => e.Close())
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

        private readonly IProperty<WindowState> _windowState = H.Property<WindowState>();
        public WindowState WindowState
        {
            get => _windowState.Get();
            set => _windowState.Set(value);
        }

        public ICommand MaximizeCommand { get; } = H.Command(c => c.Action(e =>
            {
                e.WindowState = e.WindowState != WindowState.Normal ? WindowState.Maximized : WindowState.Normal;
            }
        ));

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

        public ObservableCollection<ICommand> Commands { get; } = new ObservableCollection<ICommand>();

        public void AddButton(ICommand cmd)
        {
            Commands.Add(cmd);
        }

    }
}
