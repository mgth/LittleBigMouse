/*
  LittleBigMouse.Daemon
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Daemon.

    LittleBigMouse.Daemon is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Daemon is distributed in the hope that it will be useful,
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
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using System.Windows;
using HLab.Notify.PropertyChanged;
using HLab.Notify.Wpf;
using HLab.Remote;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.Daemon.Updater;
using LittleBigMouse.ScreenConfig;
using LittleBigMouse_Daemon.Updater;
using Microsoft.Win32.TaskScheduler;
using Task = System.Threading.Tasks.Task;

namespace LittleBigMouse.Daemon
{
    class LittleBigMouseDaemon : Application, ILittleBigMouseService
    {
        private string ServiceName { get; } = "LittleBigMouse_" + System.Security.Principal.WindowsIdentity.GetCurrent().Name.Replace('\\', '_');
        private MouseEngine _engine;
        private readonly RemoteServer _remoteServer = new RemoteServer("lbm.daemon");
        private readonly IMonitorsService _monitorsService = new MonitorsService();
        private Notify _notify;

        public LittleBigMouseDaemon()
        {
            NotifyHelper.EventHandlerService = new EventHandlerServiceWpf();
            ShutdownMode = ShutdownMode.OnExplicitShutdown;
        }


        protected override void OnStartup(StartupEventArgs e)
        {
            _notify = new Notify();
            _engine = new MouseEngine(_monitorsService);

            _engine.ConfigLoaded += _engine_ConfigLoaded;

            base.OnStartup(e);

            _remoteServer.GotMessage += OnGotMessage;
            _remoteServer.Run();

            if (_notify != null)
                _notify.Click += OnNotifyClick;

            UpdateConfig();

            _notify.AddMenu(-1, "Check for update", CheckUpdate);
            _notify.AddMenu(-1, "Open", Open);
            _notify.AddMenu(-1, "Start", Start);
            _notify.AddMenu(-1, "Stop", Stop);
            _notify.AddMenu(-1, "Exit", Quit);

            CommandLine(e.Args);
        }

        protected override void OnExit(ExitEventArgs e)
        {
            Stop();
            _remoteServer?.Stop();
            _notify.Hide();

            base.OnExit(e);
        }


        private void OnGotMessage(object sender, RemoteEventArgs e)
        {
            Application.Current.Dispatcher.Invoke(
                    () => CommandLine(e.Message)
            );
        }


        private void _engine_ConfigLoaded(object sender, EventArgs e)
        {
            UpdateConfig();
        }


        public void UpdateConfig()
        {
            _notify.RemoveMenu("config");

            foreach (string configName in ScreenConfig.ScreenConfig.ConfigsList)
            {
                bool chk = configName == _engine.Config?.Id;

                // TODO : if (_screenConfig!=null && _screenConfig.IsDoableConfig(configName))
                {
                    _notify.AddMenu(0, configName, MatchConfig, "config", chk);
                }
            }

        }


        private void OnNotifyClick(object sender, EventArgs e) { Open(); }


        private void MatchConfig(object sender, EventArgs e)
        {
            //if (sender is ToolStripMenuItem menu)
            //{
            //    _engine.MatchConfig(menu.Text);
            //}
        }


        public async void CommandLine(IList<string> args)
        {
            List<string> a = args.ToList();
            foreach (string s in a)
            {
                if (s.StartsWith("--"))
                {
                    await CommandLine(s.Substring(2));
                }
                else if (s.StartsWith("-"))
                {
                    var s1 = s.Substring(1);
                    foreach (var c in s1)
                    {
                        await CommandLine(c.ToString());
                    }
                }
                else
                {
                    await CommandLine(s);
                }
            }
        }

        public async Task CommandLine(string arg)
        {
            var args = arg.Split("=");
            switch (args[0].ToLower())
            {
                case "x":
                case "exit":
                case "quit":
                    Quit();
                    break;
                case "l":
                case "load":
                    LoadConfig();
                    break;
                case "s":
                case "start":
                    Start();
                    break;
                case "p":
                case "stop":
                    Stop();
                    break;
                case "c":
                case "schedule":
                    LoadAtStartup();
                    break;
                case "n":
                case "unschedule":
                    LoadAtStartup(false);
                    break;
                case "u":
                case "update":
                    CheckUpdateAsync();
                    break;
            }
        }

        public async void Running()
        {
            if (_engine.Running)
            {
                //await _remoteServer.SendMessageAsync("running");
            }
            else
            {
                //_server.SendMessageAsync("stopped");
            }
        }

        public async void Update()
        {
            CheckUpdateAsync();
        }

        public async void LoadAtStartup(bool state = true)
        {
            if (state) Schedule();
            else Unschedule();
        }

        //public void Init()
        //{
        //    Callback = OperationContext.Current.GetCallbackChannel<ILittleBigMouseCallback>();
        //}

        public async void LoadConfig()
        {
            _engine.LoadConfig();
            if (_engine.Config.AutoUpdate)
                CheckUpdateAsync();
        }

        private void Quit(object sender, EventArgs e) { Quit(); }
        public async void Quit()
        {
            Stop();
            Shutdown();
        }

        private async void Start(object sender, EventArgs e)
        {
            Start();
            //Callback?.OnStateChange(true);
        }
        public async void Start()
        {
            _engine.Start();
            _notify.SetOn();
            //await _server.SendMessageAsync("running");
        }

        private void Stop(object sender, EventArgs e)
        {
            Stop();
            //Callback?.OnStateChange(false);
        }
        public async void Stop()
        {
            _engine.Stop();
            _notify.SetOff();
            //await _server.SendMessageAsync("stopped");
        }

        private void Open(object sender, EventArgs eventArgs)
        {
            Open();
        }

        private void CheckUpdate(object sender, EventArgs eventArgs) { CheckUpdateAsync(); }

        private async Task<bool> CheckUpdateAsync()
        {
            var updater = new ApplicationUpdateViewModel();

            await updater.CheckVersion();

            if (updater.NewVersionFound)
            {
                var updaterView = new ApplicationUpdateView
                {
                    DataContext = updater
                };
                updaterView.ShowDialog();

                if (updater.Updated)
                {
                    Quit();
                    return true;
                }
            }

            return false;
        }


        [DllImport("user32")]
        private static extern bool SetForegroundWindow(IntPtr hwnd);
        private void Open()
        {
            Process[] pp = Process.GetProcessesByName("LittleBigMouse.Control.Loader");
            foreach (var process in pp)
            {
                SetForegroundWindow(process.MainWindowHandle);
            }
            if (pp.Length > 0) return;

            var p = Process.GetCurrentProcess();
            string filename = p.MainModule.FileName.Replace(".Daemon", ".Control.Loader").Replace(".vshost", "");

            filename = filename.Replace("\\Daemon\\", "\\Control.Loader\\");

            var startInfo = new ProcessStartInfo
            {
                FileName = filename,
                WorkingDirectory = Path.GetDirectoryName(p.MainModule.FileName),
                //Arguments = "-startcontrol",
                UseShellExecute = false
            };

            try
            {
                Process.Start(startInfo);
            }
            catch(FileNotFoundException)
            {
                //TODO
            }
        }


        public void Schedule()
        {
            using var ts = new TaskService();

            ts.RootFolder.DeleteTask(ServiceName, false);

            var td = ts.NewTask();
            td.RegistrationInfo.Description = "Multi-dpi aware monitors mouse crossover";
            td.Triggers.Add(
                //new BootTrigger());
                new LogonTrigger { UserId = System.Security.Principal.WindowsIdentity.GetCurrent().Name });

            //var p = Process.GetCurrentProcess();
            //string filename = p.MainModule.FileName.Replace(".vshost", "");
            var filename = AppDomain.CurrentDomain.BaseDirectory + AppDomain.CurrentDomain.FriendlyName.Replace(".vshost", "");

            td.Actions.Add(
                new ExecAction(filename, "--start", AppDomain.CurrentDomain.BaseDirectory)
            );

            td.Principal.RunLevel = TaskRunLevel.Highest;
            td.Settings.DisallowStartIfOnBatteries = false;
            td.Settings.DisallowStartOnRemoteAppSession = true;
            td.Settings.StopIfGoingOnBatteries = false;
            td.Settings.ExecutionTimeLimit = TimeSpan.Zero;

            ts.RootFolder.RegisterTaskDefinition(ServiceName, td);
        }
        public void Unschedule()
        {
            using var ts = new TaskService();
            ts.RootFolder.DeleteTask(ServiceName, false);
        }
    }
}
