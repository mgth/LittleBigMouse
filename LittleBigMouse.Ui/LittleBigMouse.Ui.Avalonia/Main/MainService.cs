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
using Avalonia.Controls;
using Avalonia.Threading;
using HLab.Base.Avalonia;
using HLab.Base.ReactiveUI;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Avalonia;
using HLab.Sys.Windows.Monitors;
using HLab.UserNotification;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Options;
using LittleBigMouse.Ui.Avalonia.Persistency;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Ui.Avalonia.Updater;
using LittleBigMouse.Zoning;
using Live.Avalonia;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Main;

public class LiveView(Func<Window, object> windowFactory) : ILiveView
{
    public object CreateView(Window window) => windowFactory(window);
}

public class MainService : ReactiveModel, IMainService
{
    readonly IMvvmService _mvvmService;
    readonly ISystemMonitorsService _monitors;

    readonly IUserNotificationService _notify;
    readonly ILittleBigMouseClientService _littleBigMouseClientService;
    readonly Func<MonitorsLayout> _getNewMonitorLayout;
    readonly IProcessesCollector _processesCollector;
    readonly Func<ApplicationUpdaterViewModel> _updaterLocator;


    Action<IMainPluginsViewModel>? _actions;

    readonly Func<IMainPluginsViewModel> _mainViewModelLocator;

    public IMonitorsLayout? MonitorsLayout 
    { 
        get => _monitorLayout; 
        set => this.RaiseAndSetIfChanged(ref _monitorLayout, value);
    }
    IMonitorsLayout? _monitorLayout;

    public MainService
    (
        Func<IMainPluginsViewModel> mainViewModelLocator,
        IMvvmService mvvmService,
        ILittleBigMouseClientService littleBigMouseClientService,
        IUserNotificationService notify,
        ISystemMonitorsService monitors,
        Func<MonitorsLayout> getNewMonitorLayout,
        IProcessesCollector processesCollector, 
        Func<ApplicationUpdaterViewModel> updaterLocator)
    {
        _notify = notify;
        _monitors = monitors;
        _getNewMonitorLayout = getNewMonitorLayout;
        _processesCollector = processesCollector;
        _updaterLocator = updaterLocator;
        _littleBigMouseClientService = littleBigMouseClientService;

        _mvvmService = mvvmService;
        _mainViewModelLocator = mainViewModelLocator;

        // Relate service state with notify icon
        _littleBigMouseClientService.DaemonEventReceived += (o, a) => DaemonEventReceived(o, a);
    }

    public void UpdateLayout()
    {
        // TODO : move to plugin
        if (_monitors is not SystemMonitorsService monitors) return;

        monitors.UpdateDevices();

        var old = MonitorsLayout;
        
        MonitorsLayout = _getNewMonitorLayout().UpdateFrom(monitors);
        
        old?.Dispose();
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
        viewModel.MainService = this;

        _actions?.Invoke(viewModel);

        var view = await _mvvmService
            .MainContext
            .GetViewAsync<DefaultViewMode>(viewModel, typeof(IDefaultViewClass));

        _mainWindow = view?.AsWindow() ?? throw new Exception("No window found");

        _mainWindow.Closed += (s, a) => _mainWindow = null!;

        _mainWindow.Show();

        // TODO : memory test
        Dispatcher.UIThread.RunJobs();
        GC.Collect();
    }


    public async Task StartNotifierAsync()
    {
        _notify.Click += async (s, a) => await ShowControlAsync();

        await _notify.AddMenuAsync(-1, "Check for update","Icon/lbm_on", async () => await _updaterLocator().CheckUpdateAsync(true));
        await _notify.AddMenuAsync(-1, "Open","Icon/lbm_off", ShowControlAsync);
        await _notify.AddMenuAsync(-1, "Start","Icon/Start", StartAsync);
        await _notify.AddMenuAsync(-1, "Stop","Icon/Stop", () =>
        {
            MonitorsLayout.Options.Enabled = false; 
            MonitorsLayout.SaveEnabled();
            return _littleBigMouseClientService.StopAsync();
        });
        await _notify.AddMenuAsync(-1, "Exit", "Icon/sys/Close", QuitAsync);

        await _notify.SetIconAsync("Icon/lbm_off",128);

        _notify.Show();

    }

    public void AddControlPlugin(Action<IMainPluginsViewModel>? action)
    {
        _actions += action;
    }

    async Task QuitAsync()
    {
        // TODO : it should not append by sometimes the QuitAsync does not return
        var t = Task.Delay(5000).ContinueWith(t => Dispatcher.UIThread.BeginInvokeShutdown(DispatcherPriority.Normal));
        await _littleBigMouseClientService.QuitAsync();
        Dispatcher.UIThread.BeginInvokeShutdown(DispatcherPriority.Normal);
    }

    Task StartAsync() => _littleBigMouseClientService.StartAsync(MonitorsLayout.ComputeZones());

    bool _justConnected = false;
    async Task DaemonEventReceived(object? sender, LittleBigMouseServiceEventArgs args)
    {
        switch (args.Event)
        {
            case LittleBigMouseEvent.Running:
                _justConnected = false;
                await _notify.SetIconAsync("icon/lbm_on",32);
                break;

            case LittleBigMouseEvent.Stopped:
                await _notify.SetIconAsync("icon/lbm_off",32);

                if (MonitorsLayout is not null && MonitorsLayout.Options.Enabled && _justConnected)
                {
                    _justConnected = false;
                    await StartAsync();
                }
                break;

            case LittleBigMouseEvent.Dead:
                await _notify.SetIconAsync("icon/lbm_dead",32);
                break;

            case LittleBigMouseEvent.Paused:
                await _notify.SetIconAsync("icon/lbm_paused",32);
                break;

            case LittleBigMouseEvent.SettingsChanged:
            case LittleBigMouseEvent.DesktopChanged:
            case LittleBigMouseEvent.DisplayChanged:
                await DisplayChangedAsync();
                break;

            case LittleBigMouseEvent.FocusChanged:
                _processesCollector?.AddProcess(args.Payload);
                break;
            case LittleBigMouseEvent.Connected:
                _justConnected = true;
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(args.Event), args.Event, null);
        }
    }

    bool _displayChangedTriggered = false;
    readonly object _lockDisplayChanged = new();
    async Task DisplayChangedAsync()
    {
        lock (_lockDisplayChanged)
        {
            if (_displayChangedTriggered) return;
            _displayChangedTriggered = true;
        }
        await Task.Delay(5000);
        lock (_lockDisplayChanged)
        {
            _displayChangedTriggered = false;
        }
        UpdateLayout();
        if(MonitorsLayout.Options.Enabled)
        {
            await StartAsync();
        }
    }



}