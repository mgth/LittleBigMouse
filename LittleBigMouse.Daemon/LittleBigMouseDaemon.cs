/*
  LittleBigMouse.Daemon
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

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

using HLab.Remote;
using LittleBigMouse.Zoning;
using Task = System.Threading.Tasks.Task;

namespace LittleBigMouse.Daemon
{
    class LittleBigMouseDaemon : Application, ILittleBigMouseService
    {
        private MouseEngine _engine;
        private readonly RemoteServer _remoteServer = new RemoteServer("lbm.daemon");

        public LittleBigMouseDaemon()
        {
            ShutdownMode = ShutdownMode.OnExplicitShutdown;
        }


        protected override void OnStartup(StartupEventArgs e)
        {
            _engine = new MouseEngine(null);

            base.OnStartup(e);

            _remoteServer.GotMessage += OnGotMessage;
            _remoteServer.Run();

            CommandLine(e.Args);
        }

        protected override void OnExit(ExitEventArgs e)
        {
            Stop();
            _remoteServer?.Stop();

            base.OnExit(e);
        }


        private void OnGotMessage(object sender, RemoteEventArgs e)
        {
            Application.Current.Dispatcher.Invoke(
                    () => CommandLine(e.Message)
            );
        }

//TODO implement in control

        //private void _engine_ConfigLoaded(object sender, EventArgs e)
        //{
        //    UpdateConfig();
        //}


        //public void UpdateConfig()
        //{
        //    _notify.RemoveMenu("config");

        //    foreach (string configName in Layout.LayoutsList)
        //    {
        //        bool chk = configName == _engine.Layout?.Id;

        //        // TODO : if (_screenConfig!=null && _screenConfig.IsDoableConfig(configName))
        //        {
        //            _notify.AddMenu(0, configName, MatchConfig, "config", chk);
        //        }
        //    }

        //}


        //private void MatchConfig(object sender, EventArgs e)
        //{
        //    //if (sender is ToolStripMenuItem menu)
        //    //{
        //    //    _engine.MatchConfig(menu.Text);
        //    //}
        //}


        public async void CommandLine(IList<string> args)
        {
            var a = args.ToList();
            foreach (var s in a)
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
                    break;
                case "s":
                case "start":
                    Start();
                    break;
                case "p":
                case "stop":
                    Stop();
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


        //public void Init()
        //{
        //    Callback = OperationContext.Current.GetCallbackChannel<ILittleBigMouseCallback>();
        //}

        private void Quit(object sender, EventArgs e) { Quit(); }
        public async void Quit()
        {
            Stop();
            Shutdown();
        }

        public void Start(ZonesLayout layout)
        {
            throw new NotImplementedException();
        }

        private async void Start(object sender, EventArgs e)
        {
            Start();
            //Callback?.OnStateChange(true);
        }
        public async void Start()
        {
            _engine.Start();
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
            //await _server.SendMessageAsync("stopped");
        }


        // TODO implement in control

        //private void CheckUpdate(object sender, EventArgs eventArgs) { CheckUpdateAsync(); }

        //private async Task<bool> CheckUpdateAsync()
        //{
        //    var updater = new ApplicationUpdateViewModel();

        //    await updater.CheckVersion();

        //    if (updater.NewVersionFound)
        //    {
        //        var updaterView = new ApplicationUpdateView
        //        {
        //            DataContext = updater
        //        };
        //        updaterView.ShowDialog();

        //        if (updater.Updated)
        //        {
        //            Quit();
        //            return true;
        //        }
        //    }

        //    return false;
        //}



        // TODO implement in control
        //public void Schedule()
        //{
        //    using var ts = new TaskService();

        //    ts.RootFolder.DeleteTask(ServiceName, false);

        //    var td = ts.NewTask();
        //    td.RegistrationInfo.Description = "Multi-dpi aware monitors mouse crossover";
        //    td.Triggers.Add(
        //        //new BootTrigger());
        //        new LogonTrigger { UserId = System.Security.Principal.WindowsIdentity.GetCurrent().Name });

        //    //var p = Process.GetCurrentProcess();
        //    //string filename = p.MainModule.FileName.Replace(".vshost", "");
        //    var filename = AppDomain.CurrentDomain.BaseDirectory + AppDomain.CurrentDomain.FriendlyName.Replace(".vshost", "");

        //    td.Actions.Add(
        //        new ExecAction(filename, "--start", AppDomain.CurrentDomain.BaseDirectory)
        //    );

        //    td.Principal.RunLevel = TaskRunLevel.Highest;
        //    td.Settings.DisallowStartIfOnBatteries = false;
        //    td.Settings.DisallowStartOnRemoteAppSession = true;
        //    td.Settings.StopIfGoingOnBatteries = false;
        //    td.Settings.ExecutionTimeLimit = TimeSpan.Zero;

        //    ts.RootFolder.RegisterTaskDefinition(ServiceName, td);
        //}
        //public void Unschedule()
        //{
        //    using var ts = new TaskService();
        //    ts.RootFolder.DeleteTask(ServiceName, false);
        //}
    }
}
