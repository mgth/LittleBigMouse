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
using System.Diagnostics;
using System.Threading.Tasks;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Avalonia;
using HLab.Sys.Windows.Monitors;
using HLab.UserNotification;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Zoning;
using Live.Avalonia;

namespace LittleBigMouse.Ui.Avalonia.Main;

public class LiveView : ILiveView
{
    readonly Func<Window, object> _windowFactory;

    public LiveView(Func<Window, object> windowFactory)
    {
        _windowFactory = windowFactory;
    }

    public object CreateView(Window window) => _windowFactory(window);
}

public class MainService : IMainService
{

    readonly IMvvmService _mvvmService;
    readonly IMonitorsSet _monitorsSet;

    readonly IUserNotificationService _notify;
    readonly ILittleBigMouseClientService _littleBigMouseClientService;

    Action<IMainPluginsViewModel>? _actions;

    readonly DisplayChangeMonitor _listener = new DisplayChangeMonitor();
    readonly IApplicationLifetime _app;

    readonly Func<IMainPluginsViewModel> _mainViewModelLocator;

    public MainService(
        Func<IMainPluginsViewModel> mainViewModelLocator,
        IMvvmService mvvmService,
        ILittleBigMouseClientService littleBigMouseClientService,
        IUserNotificationService notify,
        IMonitorsSet monitorsService,
        IApplicationLifetime app)
    {
        _notify = notify;
        _monitorsSet = monitorsService;
        _app = app;
        _littleBigMouseClientService = littleBigMouseClientService;

        _mvvmService = mvvmService;
        _mainViewModelLocator = mainViewModelLocator;

        // Relate service state with notify icon
        _littleBigMouseClientService.StateChanged += _littleBigMouseClientService_StateChanged;

    }

    public async Task ShowControlAsync()
    {

        if (_app is IClassicDesktopStyleApplicationLifetime { MainWindow.IsLoaded: true } desktop)
        {
            desktop.MainWindow.Activate();
            return;
        }

        IMonitorsLayout layout = new MonitorsLayout();
        // TODO : move to plugin
        if (_monitorsSet is MonitorsService s && layout is MonitorsLayout l)
        {
            s.UpdateDevices();
            l.UpdateFrom(s);

            _listener.DisplayChanged += (o, a) =>
            {
                s.UpdateDevices();
                l.UpdateFrom(s);
            };
        }

        var viewModel = _mainViewModelLocator();
        viewModel.Content = layout;

        _actions?.Invoke(viewModel);


#if DEBUG
        if (Debugger.IsAttached)
        {
#endif
                var window = (await _mvvmService
                    .MainContext
                    .GetViewAsync<DefaultViewMode>(viewModel, typeof(IDefaultViewClass)))
                    ?.AsWindow();

                if (window == null) return;

                if (_app is IClassicDesktopStyleApplicationLifetime d)
                {
                    d.MainWindow = window;
                    window.Closed += (s, a) => d.MainWindow = null;
                }

                window.Show();
#if DEBUG
        }
        else
        {
            var ctx = _mvvmService.MainContext;

            var liveView = new LiveView(
                w => (ctx.GetViewAsync<DefaultViewMode>(viewModel, typeof(IDefaultViewClass)))?.Result);

            var window = new LiveViewHost(liveView, Console.WriteLine);
            window.StartWatchingSourceFilesForHotReloading();

            if (_app is IClassicDesktopStyleApplicationLifetime d)
            {
                d.MainWindow = window;
                window.Closed += (s, a) => d.MainWindow = null;
            }

            window.Show();
        }
#endif

    }


    public void StartNotifier()
    {
        _notify.Click += async (s, a) => await ShowControlAsync();

        // TODO : _notify.AddMenu(-1, "Check for update", CheckUpdate);
        _notify.AddMenu(-1, "Open","Icon/Start", ShowControlAsync);
        // TODO : _notify.AddMenu(-1, "Start","Icon/Start", () => _littleBigMouseClientService.StartAsync(Layout.ComputeZones()));
        _notify.AddMenu(-1, "Stop","Icon/Stop", () => _littleBigMouseClientService.StopAsync());
        _notify.AddMenu(-1, "Exit", "Icon/Stop", QuitAsync);

        _notify.SetIcon("icon/lbm_off",128);
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
    }
    // TODO : Avalonia
        //Application.Current.Shutdown();

    void _littleBigMouseClientService_StateChanged(object? sender, LittleBigMouseServiceEventArgs args)
    {
        switch (args.State)
        {
            case LittleBigMouseState.Running:
                _notify.SetIcon("icon/lbm_on",32);
                break;
            case LittleBigMouseState.Stopped:
            case LittleBigMouseState.Dead:
                _notify.SetIcon("icon/lbm_off",32);
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(args.State), args.State, null);
        }
    }
}