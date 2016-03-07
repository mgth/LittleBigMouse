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
    [ServiceBehavior(ConcurrencyMode = ConcurrencyMode.Reentrant, InstanceContextMode = InstanceContextMode.Single)]
    class LittleBigMouseDaemon : Application, ILittleBigMouseService
    {
        private const string ServiceName = "LittleBigMouse";
        private MouseEngine _engine;
        private Notify _notify;

        public LittleBigMouseDaemon()
        {

            ShutdownMode = ShutdownMode.OnExplicitShutdown;
            Startup += OnStartup;
            Exit += OnExit;
            Deactivated += OnDeactivated; 

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

            _notify.AddMenu(-1,"Open", Open);
            _notify.AddMenu(-1,"Start", Start);
            _notify.AddMenu(-1,"Stop", Stop);
            _notify.AddMenu(-1,"Exit", Quit);

            Start();
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
            _notify.Hide();
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

        public bool Running()
        {
            return _engine.Hook.Enabled;
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
