using MouseKeyboardActivityMonitor;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows;
using LittleBigMouseGeo;
using Microsoft.Win32;
using MouseKeyboardActivityMonitor.WinApi;
using LbmScreenConfig;
using System.ServiceModel;
using System.ServiceModel.Description;
using System.Threading;
using Microsoft.Win32.TaskScheduler;

namespace LittleBigMouse_Daemon
{
    [ServiceBehavior(InstanceContextMode = InstanceContextMode.Single)]
    class Program : Application, ILittleBigMouseService
    {
        private const string Unique = "LittleBigMouse_Daemon";
        [STAThread]
        public static void Main(string[] args)
        {
            bool firstInstance;
            Mutex mutex = new Mutex(true, "LittleBigMouse_Daemon" + Environment.UserName, out firstInstance);

            if (!firstInstance)
            {
                LittleBigMouseClient.Client.CommandLine(args);
                mutex.Close();
                return;
            }


            Program prog = new Program();

            using (ServiceHost host = new ServiceHost(prog, LittleBigMouseClient.Address))
            {
                // Enable metadata publishing.
                ServiceMetadataBehavior smb = new ServiceMetadataBehavior();
                //smb.HttpGetEnabled = true;
                smb.MetadataExporter.PolicyVersion = PolicyVersion.Policy15;

                host.Description.Behaviors.Add(smb);

                host.Open();

                prog.Run();

                host.Close();
            }

            mutex.Close();
        }

        private readonly Notify _notify;
        public Program()
        {
            _notify = new Notify();
            if (_notify!=null)
                _notify.Click += _notify_Click;

            _notify.AddMenu("Open", Open);
            _notify.AddMenu("Start", Start);
            _notify.AddMenu("Stop", Stop);
            _notify.AddMenu("Exit", Quit);


            SystemEvents.DisplaySettingsChanged += SystemEvents_DisplaySettingsChanged;
            this.Exit += Program_Exit;

            Start();
        }

        private void Open(object sender, EventArgs eventArgs) { Open(); }
        private void _notify_Click(object sender, EventArgs e) { Open(); }
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


        private void Program_Exit(object sender, ExitEventArgs e)
        {
            _notify.Dispose();
        }

        public void CommandLine(IList<string> args)
        {
            foreach (string s in args)
            {
                switch (s)
                {
                    case "--exit":
                        Stop();
                        Shutdown();
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
                        LoadAtStartup(true);
                        break;
                    case "--unschedule":
                        LoadAtStartup(false);
                        break;
                }
            }
        }

        private ScreenConfig _config;

        private readonly MouseHookListener _mouseHookManager = new MouseHookListener(new GlobalHooker());
        private List<PixelPoint> _oldPoints = new List<PixelPoint>();

        public void Quit()
        {
            Stop();
            Shutdown();
        }

        private void Start(object sender, EventArgs e) { Start(); }
        public void Start()
        {
            if (_mouseHookManager.Enabled) return;

            if (_config == null)
                LoadConfig();

            if (_config == null || !_config.Enabled) return;

            _mouseHookManager.MouseMoveExt += _MouseHookManager_MouseMoveExt;
            _mouseHookManager.Enabled = true;

            _notify.SetOn();
        }
        private void Stop(object sender, EventArgs e) {  Stop(); }
        public void Stop()
        {
            if (!_mouseHookManager.Enabled) return;

            _mouseHookManager.MouseMoveExt -= _MouseHookManager_MouseMoveExt;
            _mouseHookManager.Enabled = false;

            if (_config != null)
            {
                //todo : save initial configuration to be restored
                if (_config.AdjustSpeed)
                    Mouse.MouseSpeed = 10.0;

                if (_config.AdjustPointer)
                    Mouse.SetCursorAero(1);
            }

            _notify.SetOff();
        }

        public void LoadAtStartup(bool state = true)
        {
            if (state) Schedule();
            else Unschedule();
        }

        public void CommandLine(string[] args)
        {
            throw new NotImplementedException();
        }

        public void LoadConfig(ScreenConfig config)
        {
            _config = config;
        }

        private void Quit(object sender, EventArgs e) { Quit(); }

        private void SystemEvents_DisplaySettingsChanged(object sender, EventArgs e)
        {
            LoadConfig();
        }
        public void LoadConfig()
        {
            LoadConfig(new ScreenConfig());
        }
        private const string ServiceName = "LittleBigMouse";

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

                var p = Process.GetCurrentProcess();
                string filename = p.MainModule.FileName.Replace(".vshost", "");


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

        private void AddOldPoint(PixelPoint p)
        {
            _oldPoints.Add(p);
            if (_oldPoints.Count > 3) _oldPoints.Remove(_oldPoints.First());
        }

        private void _MouseHookManager_MouseMoveExt(object sender, MouseEventExtArgs e)
        {
            // If first time called just save that point
            if (_oldPoints.Count == 0)
            {
                AddOldPoint(new PixelPoint(_config, null, e.X, e.Y));
                return;
            }


            if (e.Clicked) return;

            PixelPoint pIn = new PixelPoint(
                _oldPoints.Last().Config,
                _oldPoints.Last().Screen, e.X, e.Y);

            // No move
            if (pIn.Equals(_oldPoints.Last())) return;

            // no screen change
            if (pIn.TargetScreen == _oldPoints.Last().Screen)
            {
                AddOldPoint(pIn);
                return;
            }

            Screen screenOut = pIn.Physical.TargetScreen;


            //Debug.WriteLine("From:" + _oldPoint.Screen.DeviceName + " X:" + _oldPoint.X + " Y:" + _oldPoint.Y);
            //Debug.WriteLine(" To:" + (screenOut?.DeviceName ?? "null") + " X:" + pIn.X + " Y:" + pIn.Y + "\n");


            //
            // Allow Corner Jump
            //
            if (screenOut == null)
            {
                double dist = 100.0;
                Segment seg = new Segment(_oldPoints.First().Physical.Point, pIn.Physical.Point);

                Debug.Print(seg.Line.Coef.ToString());

                // Calculate side to enter screen when corner crossing not allowed.
                Side side = Segment.OpositeSide(seg.IntersectSide(_oldPoints.First().Screen.PhysicalBounds));

                foreach (Screen screen in _config.AllScreens.Where(s => s!= _oldPoints.Last().Screen))
                {
                    foreach (
                        Point p in 
                        _config.AllowCornerCrossing?
                        seg.Line.Intersect(screen.PhysicalBounds):
                        seg.Line.Intersect(screen.PhysicalBounds.Segment(side))
                    )
                    {
                        Segment travel = new Segment(_oldPoints.Last().Physical.Point, p);
                        if (!travel.Rect.Contains(pIn.Physical.Point)) continue;
                        if (travel.Size > dist) continue;

                        dist = travel.Size;
                        pIn = (new PhysicalPoint(_config, screen, p.X, p.Y)).Pixel.Inside;
                        screenOut = screen;
                    }
                }                
            }

            // if new position is not within another screen
            if (screenOut == null)
            {
                Mouse.CursorPos = pIn.Inside./*MouseWpf.*/Point;
                e.Handled = true;
                return;
            }

            // Mouse.CursorPos = screenOut.PixelLocation.Point;

            // Actual mouving mouse to new location
            Mouse.CursorPos = pIn.Physical.ToScreen(screenOut).Pixel./*Inside.MouseWpf.*/Point;

            // Adjust pointer size to dpi ratio : should not be usefull if windows screen ratio is used
            if (_config.AdjustPointer)
            {
                if (screenOut.RealDpiAvg > 110)
                {
                    Mouse.SetCursorAero(screenOut.RealDpiAvg > 138 ? 3 : 2);
                }
                else Mouse.SetCursorAero(1);
            }


            // Adjust pointer speed to dpi ratio : should not be usefull if windows screen ratio is used
            if (_config.AdjustSpeed)
            {
                Mouse.MouseSpeed = Math.Round((5.0 / 96.0) * screenOut.RealDpiAvg, 0);
            }

            AddOldPoint(new PixelPoint(_config, screenOut, pIn.X, pIn.Y));
            e.Handled = true;
        }

    }


}
