/*
  LittleBigMouse.Control.Core
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

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
using System.Windows.Input;
using Avalonia.Controls;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Avalonia.MonitorFrame;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia;

public class MonitorsLayoutPresenterViewModel 
    : ViewModel<IMonitorsLayout>
        , IMvvmContextProvider
        , IMonitorsLayoutPresenterViewModel
{
    public IMainPluginsViewModel MainViewModel { get; }

    public MonitorsLayoutPresenterViewModel(IMainPluginsViewModel mainViewModel)
    {
        MainViewModel = mainViewModel;

        _width = this.WhenAnyValue(
                e => e.Model.PhysicalBounds.Width,
                e => e.VisualRatio.X,
                (w, x) => w * x)
            .ToProperty(this, e => e.Width);

        _height = this.WhenAnyValue(
                e => e.Model.PhysicalBounds.Height,
                e => e.VisualRatio.Y,
                (h, y) => h * y)
            .ToProperty(this, e => e.Height);

        ResetLocationsFromSystem = ReactiveCommand.Create(() => Model?.SetLocationsFromSystemConfiguration());
        ResetSizesFromSystem = ReactiveCommand.Create(() => Model?.SetSizesFromSystemConfiguration());

        RefreshCommand = ReactiveCommand.Create(()=> this.test());
    }

    void test()
    {

    }

    public MonitorsLayoutPresenterViewModel():this(new MainViewModelDesign())
    {
        if(!Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");
    }

    public double Width => _width.Value;
    readonly ObservableAsPropertyHelper<double> _width;

    public double Height => _height.Value;
    readonly ObservableAsPropertyHelper<double> _height;

    public IDisplayRatio VisualRatio { get; } = new DisplayRatioValue(1.0);

    public IMonitorFrameViewModel? SelectedMonitor
    {
        get => _selectedMonitor;
        set => this.RaiseAndSetIfChanged(ref _selectedMonitor, value);
    }
    private IMonitorFrameViewModel? _selectedMonitor;

    public ICommand ResetLocationsFromSystem { get; }
    public ICommand ResetSizesFromSystem { get; }
    public ICommand RefreshCommand { get; }

    public void ConfigureMvvmContext(IMvvmContext ctx)
    {
        ctx.AddCreator<MonitorFrameViewModel>(vm => vm.MonitorsPresenter = this);
    }
}