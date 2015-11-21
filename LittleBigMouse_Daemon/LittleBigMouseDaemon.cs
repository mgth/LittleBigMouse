using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.ServiceModel;
using System.Windows;
using System.Windows.Forms;
using LbmScreenConfig;
using Microsoft.Win32.TaskScheduler;
using Application = System.Windows.Application;

namespace LittleBigMouse_Daemon
{
    [ServiceBehavior(InstanceContextMode = InstanceContextMode.Single)]
    class LittleBigMouseDaemon : Application, ILittleBigMouseService
    {
        private const string ServiceName = "LittleBigMouse";
        private MouseEngine _engine;
        private Notify _notify;

        public LittleBigMouseDaemon()
        {
            Startup += OnStartup;
            Exit += OnExit;

        }

        private void OnStartup(object sender, EventArgs exitEventArgs)
        {
            _notify = new Notify();
            _engine = new MouseEngine();

            _engine.StartServer(this);

            if (_notify != null)
                _notify.Click += OnNotifyClick;

            foreach (string configName in ScreenConfig.ConfigsList)
            {
                _notify.AddMenu(configName, MatchConfig);
            }


            _notify.AddMenu("Open", Open);
            _notify.AddMenu("Start", Start);
            _notify.AddMenu("Stop", Stop);
            _notify.AddMenu("Exit", Quit);

            Start();
        }

        private void MatchConfig(object sender, EventArgs e)
        {
            var menu = sender as ToolStripMenuItem;
            if (menu != null)
            {
                _engine.MatchConfig(menu.Text);
            }
        }

        private void OnExit(object sender, ExitEventArgs exitEventArgs)
        {
            Stop();
            _engine.StopServer();
            _notify.Dispose();
        }

        public void CommandLine(IList<string> args)
        {
            foreach (string s in args)
            {
                switch (s)
                {
                    case "--exit":
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
                }
            }
        }

        public void LoadAtStartup(bool state = true)
        {
            if (state) Schedule();
            else Unschedule();
        }

        public void LoadConfig()
        {
            _engine.LoadConfig();
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
        private void OnNotifyClick(object sender, EventArgs e) { Open(); }
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
            string filename = p.MainModule.FileName.Replace("Daemon", "Control").Replace(".vshost", "");
            Process.Start(filename, "--startcontrol");
        }
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
                    new ExecAction(filename)
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
