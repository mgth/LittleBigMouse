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
using System.Threading.Tasks;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Threading;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Avalonia;
using HLab.Sys.Windows.Monitors;
using HLab.UserNotification;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Updater;
using LittleBigMouse.Zoning;
using Live.Avalonia;

namespace LittleBigMouse.Ui.Avalonia.Main;

public class LiveView(Func<Window, object> windowFactory) : ILiveView
{
    public object CreateView(Window window) => windowFactory(window);
}

public class MainService : IMainService
{

    readonly IMvvmService _mvvmService;
    readonly IMonitorsSet _monitorsSet;

    readonly IUserNotificationService _notify;
    readonly ILittleBigMouseClientService _littleBigMouseClientService;

    Action<IMainPluginsViewModel>? _actions;

    readonly DisplayChangeMonitor _listener = new DisplayChangeMonitor();

    readonly Func<IMainPluginsViewModel> _mainViewModelLocator;

    public IMonitorsLayout MonitorsLayout { get; } = new MonitorsLayout();

    public MainService(
        Func<IMainPluginsViewModel> mainViewModelLocator,
        IMvvmService mvvmService,
        ILittleBigMouseClientService littleBigMouseClientService,
        IUserNotificationService notify,
        IMonitorsSet monitorsService
        )
    {
        _notify = notify;
        _monitorsSet = monitorsService;
        _littleBigMouseClientService = littleBigMouseClientService;

        _mvvmService = mvvmService;
        _mainViewModelLocator = mainViewModelLocator;

        // Relate service state with notify icon
        _littleBigMouseClientService.StateChanged += (o,a) => StateChanged(o,a);

    }

    public void UpdateLayout()
    {
        // TODO : move to plugin
        if (_monitorsSet is MonitorsService s && MonitorsLayout is MonitorsLayout l)
        {
            s.UpdateDevices();
            l.UpdateFrom(s);

            _listener.DisplayChanged += (o, a) =>
            {
                s.UpdateDevices();
                l.UpdateFrom(s);
            };
        }
    }

    Window _mainWindow = null!;

    public async Task ShowControlAsync()
    {

        if (_mainWindow?.IsLoaded == true)
        {
            _mainWindow.WindowState = WindowState.Normal;
            _mainWindow.Activate();
            return;
        }

        var viewModel = _mainViewModelLocator();
        viewModel.Content = MonitorsLayout;

        _actions?.Invoke(viewModel);

        var view = await _mvvmService
            .MainContext
            .GetViewAsync<DefaultViewMode>(viewModel, typeof(IDefaultViewClass));

        _mainWindow = view?.AsWindow() ?? throw new Exception("No window found");

        _mainWindow.Closed += (s, a) => _mainWindow = null!;

        _mainWindow.Show();
    }


    public async Task StartNotifierAsync()
    {
        _notify.Click += async (s, a) => await ShowControlAsync();

        await _notify.AddMenuAsync(-1, "Check for update","Icon/lbm_on", CheckUpdateAsync);
        await _notify.AddMenuAsync(-1, "Open","Icon/lbm_off", ShowControlAsync);
        await _notify.AddMenuAsync(-1, "Start","Icon/Start", () => _littleBigMouseClientService.StartAsync(MonitorsLayout.ComputeZones()));
        await _notify.AddMenuAsync(-1, "Stop","Icon/Stop", () => _littleBigMouseClientService.StopAsync());
        await _notify.AddMenuAsync(-1, "Exit", "Icon/sys/Close", QuitAsync);

        //await _notify.SetIconAsync("Icon/Stop",128);
        await _notify.SetIconAsync("Icon/lbm_off",128);
        //_notify.SetIcon("icon/MonitorLocation",128);
        //_notify.SetIcon(Resources.lbm_off);
        _notify.Show();
    }

    public void AddControlPlugin(Action<IMainPluginsViewModel>? action)
    {
        _actions += action;
    }

    async Task QuitAsync()
    {
        await _littleBigMouseClientService.QuitAsync();
        Dispatcher.UIThread.BeginInvokeShutdown(DispatcherPriority.Normal);
    }

    async Task StateChanged(object? sender, LittleBigMouseServiceEventArgs args)
    {
        switch (args.State)
        {
            case LittleBigMouseState.Running:
                await _notify.SetIconAsync("icon/lbm_on",32);
                break;
            case LittleBigMouseState.Stopped:
            case LittleBigMouseState.Dead:
                await _notify.SetIconAsync("icon/lbm_off",32);
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(args.State), args.State, null);
        }
    }

    async Task CheckUpdateAsync()
    {
        var updater = new ApplicationUpdaterViewModel();

        var updaterView = new ApplicationUpdaterView
        {
            DataContext = updater
        };
        updaterView.Show();

        await updater.CheckVersion();

        if (updater.NewVersionFound)
        {

            if (updater.Updated)
            {
                //Application.Current.Shutdown();
                return;
            }
        }
    }

    async Task CheckUpdateBlindAsync()
    {
        var updater = new ApplicationUpdaterViewModel();

        await updater.CheckVersion();

        if (updater.NewVersionFound)
        {
            var updaterView = new ApplicationUpdaterView
            {
                DataContext = updater
            };
            updaterView.Show();

            if (updater.Updated)
            {
                //Application.Current.Shutdown();
                return;
            }
        }
    }

}