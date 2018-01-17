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
using System.Linq;
using System.Runtime.InteropServices;
using System.ServiceModel;
using System.Windows;
using System.Windows.Forms;
using LittleBigMouse.ScreenConfigs;
using LittleBigMouse_Daemon.Updater;
using Microsoft.Win32.TaskScheduler;
using Application = System.Windows.Application;

namespace LittleBigMouse_Daemon
{
    [ServiceBehavior(ConcurrencyMode = ConcurrencyMode.Reentrant, InstanceContextMode = InstanceContextMode.Single)]
    class LittleBigMouseDaemon : Application, ILittleBigMouseService
    {
        private const string ServiceName = "LittleBigMouse";
        private MouseEngine _engine;
        private Notify _notify;
        private readonly IList<string> _args;

        public LittleBigMouseDaemon(IList<string> args)
        {

            ShutdownMode = ShutdownMode.OnExplicitShutdown;
            Startup += OnStartup;
            Exit += OnExit;
            Deactivated += OnDeactivated;
            _args = args;
        }

       // public static ILittleBigMouseCallback Callback;

        private void OnStartup(object sender, EventArgs exitEventArgs)
        {

            _notify = new Notify();
            _engine = new MouseEngine();

            _engine.ConfigLoaded += _engine_ConfigLoaded;

            _engine.StartServer(this);

            

            if (_notify != null)
                _notify.Click += OnNotifyClick;

            UpdateConfig();

            //_notify.AddMenu("Brightness", Brightness);

            _notify.AddMenu(-1,"Check for update",CheckUpdate);
            _notify.AddMenu(-1,"Open", Open);
            _notify.AddMenu(-1,"Start", Start);
            _notify.AddMenu(-1,"Stop", Stop);
            _notify.AddMenu(-1,"Exit", Quit);

            CommandLine(_args);
            //Start();
        }

        private void _engine_ConfigLoaded(object sender, EventArgs e)
        {
            UpdateConfig();
        }

        public void UpdateConfig()
        {
            _notify.RemoveMenu("config");

            foreach (string configName in ScreenConfig.ConfigsList)
            {
                bool chk = configName==_engine.Config?.Id;

                if (ScreenConfig.IsDoableConfig(configName))
                    _notify.AddMenu(0,configName, MatchConfig, "config", chk);
            }

        }


        private void OnNotifyClick(object sender, EventArgs e) { Open(); }

        private void OnDeactivated(object sender, EventArgs eventArgs)
        {
            //_brightness?.Hide();
        }

        private void MatchConfig(object sender, EventArgs e)
        {
            if (sender is ToolStripMenuItem menu)
            {
                _engine.MatchConfig(menu.Text);
            }
        }

        private void OnExit(object sender, ExitEventArgs exitEventArgs)
        {
            Stop();
            _engine.StopServer();
            _notify.Hide();
        }

        public void CommandLine(IList<string> args)
        {
            List<string> a = args.ToList();
            foreach (string s in a)
            {
                //args.Remove(s);
                switch (s.ToLower())
                {
                    case "--exit":
                    case "--quit":
                        Quit();
                        break;
                    case "--load":
                        LoadConfig();
                        break;
                    case "--start":
                        Start();
                        break;
                    case "--stop":
                        Stop();
                        break;
                    case "--schedule":
                        LoadAtStartup();
                        break;
                    case "--unschedule":
                        LoadAtStartup(false);
                        break;
                    case "--update":
                        CheckUpdateAsync();
                        break;
                }
            }
        }

        public bool Running()
        {
            return _engine.Hook.Enabled;
        }

        public void Update()
        {
            CheckUpdateAsync();
        }

        public void LoadAtStartup(bool state = true)
        {
            if (state) Schedule();
            else Unschedule();
        }

        //public void Init()
        //{
        //    Callback = OperationContext.Current.GetCallbackChannel<ILittleBigMouseCallback>();
        //}

        public void LoadConfig()
        {
            _engine.LoadConfig();
            if (_engine.Config.AutoUpdate)
                CheckUpdateAsync();
        }

        private void Quit(object sender, EventArgs e) { Quit(); }
        public void Quit()
        {
            Stop();
            Shutdown();
        }

        private void Start(object sender, EventArgs e) { Start(); }
        public void Start()
        {
            _engine.Start();
            _notify.SetOn();
        }

        private void Stop(object sender, EventArgs e) { Stop(); }
        public void Stop()
        {
            _engine.Stop();
            _notify.SetOff();
        }
        private void Open(object sender, EventArgs eventArgs) { Open(); }

        private void CheckUpdate(object sender, EventArgs eventArgs) { CheckUpdateAsync(); }

        private async System.Threading.Tasks.Task CheckUpdateAsync()
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
                    Application.Current.Shutdown();
                    return;
                }
            }
        }


        [DllImport("user32")]
        private static extern bool SetForegroundWindow(IntPtr hwnd);
        private void Open()
        {
            Process[] pp = Process.GetProcessesByName("LittleBigMouse_Control");
            foreach (var process in pp)
            {
                SetForegroundWindow(process.MainWindowHandle);
            }
            if (pp.Length > 0) return;

            var p = Process.GetCurrentProcess();
            string filename = p.MainModule.FileName.Replace("_Daemon", "_Control").Replace(".vshost", "");
            Process.Start(filename, "--startcontrol");
        }

        /*
        private readonly LuminanceWindow _brightness = new LuminanceWindow();
        private void OnNotifyClick(object sender, EventArgs e) { Brightness(); }

        public void Brightness(object sender, EventArgs eventArgs)
        {
            Brightness();
        }
        private void Brightness()
        {
            if (_brightness == null) return;

            if (_brightness.Visibility == Visibility.Visible)
                _brightness.Hide();
            else
            {
                _brightness.Hook = _engine.Hook;
                _brightness.Show();
                
            }
        }
        */

        public void Schedule()
        {
            using (TaskService ts = new TaskService())
            {
                ts.RootFolder.DeleteTask(ServiceName, false);

                TaskDefinition td = ts.NewTask();
                td.RegistrationInfo.Description = "Multi-dpi aware monitors mouse crossover";
                td.Triggers.Add(
                    //new BootTrigger());
                    new LogonTrigger());

                //var p = Process.GetCurrentProcess();
                //string filename = p.MainModule.FileName.Replace(".vshost", "");
                string filename = AppDomain.CurrentDomain.BaseDirectory + AppDomain.CurrentDomain.FriendlyName.Replace(".vshost", "");

                td.Actions.Add(
                    new ExecAction(filename,"--start")
                    );

                td.Principal.RunLevel = TaskRunLevel.Highest;
                td.Settings.DisallowStartIfOnBatteries = false;
                td.Settings.DisallowStartOnRemoteAppSession = true;
                td.Settings.StopIfGoingOnBatteries = false;
                td.Settings.ExecutionTimeLimit = TimeSpan.Zero;

                ts.RootFolder.RegisterTaskDefinition(ServiceName, td);
            }
        }
        public void Unschedule()
        {
            using (TaskService ts = new TaskService())
            {
                ts.RootFolder.DeleteTask(ServiceName, false);
            }

        }

    }
}
