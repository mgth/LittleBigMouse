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
using System.Linq;
using System.Threading.Tasks;
using System.Windows.Input;
using Avalonia.Controls;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Controls;
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

    // Nullable: only the greedy ctor is resolved by the DI container at runtime; the
    // design-mode path never invokes the apply command (same pattern as DefaultMonitorViewModel).
    readonly IDisplayController? _controller;
    readonly ILayoutPersistence? _persistence;

    public MonitorsLayoutPresenterViewModel(IMainPluginsViewModel mainViewModel)
        : this(mainViewModel, null, null)
    {
    }

    public MonitorsLayoutPresenterViewModel(
        IMainPluginsViewModel mainViewModel,
        IDisplayController? controller,
        ILayoutPersistence? persistence)
    {
        MainViewModel = mainViewModel;
        _controller = controller;
        _persistence = persistence;

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
        ApplyLocationsToSystem = ReactiveCommand.CreateFromTask<Window?>(ApplyLocationsToSystemAsync);

        RefreshCommand = ReactiveCommand.Create(test);
    }

    void test()
    {

    }

    /// <summary>
    /// Push the physical layout to the system display configuration (inverse of
    /// <see cref="MonitorsLocationsFromSystemExtensions.SetLocationsFromSystemConfiguration"/>).
    /// </summary>
    async Task ApplyLocationsToSystemAsync(Window? owner)
    {
        if (Model is not MonitorsLayout layout || _controller is null) return;

        var (confirmed, adjustScale) = await MonitorWarningDialog.ShowApplyLayoutAsync(
            owner, offerScale: OperatingSystem.IsLinux());
        if (!confirmed) return;

        // No pre-filtering: the whole topology goes to the controller, which may need to
        // translate it (kscreen refuses negative positions) before it can tell what
        // actually changes.
        var locations = layout.ComputePixelLocationsFromPhysical(adjustScale)
            .Select(kv => (kv.Key, kv.Value.PixelBounds.Location, kv.Value.Scale))
            .ToList();

        if (locations.Count == 0) return;

        // The system change triggers a rebuild that re-imports the system layout and then
        // loads the saved one: the current physical layout survives only if saved first.
        _persistence?.Save(layout);

        _controller.SetLocations(locations);
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

    public PhysicalMonitor? SelectedMonitor { get; set => this.RaiseAndSetIfChanged(ref field, value);}

    public ICommand ResetLocationsFromSystem { get; }
    public ICommand ResetSizesFromSystem { get; }
    public ICommand ApplyLocationsToSystem { get; }
    public ICommand RefreshCommand { get; }

    public void ConfigureMvvmContext(IMvvmContext ctx)
    {
        ctx.AddCreator<MonitorFrameViewModel>(vm => vm.MonitorsPresenter = this);
    }
}