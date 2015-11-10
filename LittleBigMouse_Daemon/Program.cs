using MouseKeyboardActivityMonitor;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Diagnostics.Eventing.Reader;
using System.Linq;
using System.Threading;
using System.Windows;
using System.Windows.Controls;
using LittleBigMouseGeo;
using Microsoft.Win32;
using MouseKeyboardActivityMonitor.WinApi;
using LbmScreenConfig;
using Microsoft.Shell;

namespace LittleBigMouse_Daemon
{
    class Program : Application, ISingleInstanceApp
    {
        private const string Unique = "LittleBigMouse_Daemon";
        [STAThread]
        public static void Main(string[] args)
        {
            if (SingleInstance<Program>.InitializeAsFirstInstance(Unique))
            {
                var prog = new Program {ShutdownMode = ShutdownMode.OnExplicitShutdown};

                prog.Run();

                // Allow single instance code to perform cleanup operations
                SingleInstance<Program>.Cleanup();
            }
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
            _notify.AddMenu("Exit", ExitProg);


            SystemEvents.DisplaySettingsChanged += SystemEvents_DisplaySettingsChanged;
            this.Exit += Program_Exit;


        }

        private void Open(object sender, EventArgs eventArgs)
        {
            var p = Process.GetCurrentProcess();
            string filename = p.MainModule.FileName.Replace("Daemon", "StartUp").Replace(".vshost", "");
            Process.Start(filename, "--startcontrol");
        }

        private static void _notify_Click(object sender, EventArgs e)
        {

        }

        private void Program_Exit(object sender, ExitEventArgs e)
        {
            _notify.Dispose();
        }

        public bool SignalExternalCommandLineArgs(IList<string> args)
        {
            ParseCommandLine(args);
            return true;
        }

        public void ParseCommandLine(IList<string> args)
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
                        _config = new ScreenConfig();
                        break;
                    case "--start":
                        Start();
                        break;
                    case "--stop":
                        Stop();
                        break;
                }
            }
        }

        private ScreenConfig _config;

        private readonly MouseHookListener _mouseHookManager = new MouseHookListener(new GlobalHooker());
        private PixelPoint _oldPoint;

        private void Start(object sender, EventArgs e) { Start(); }
        public void Start()
        {
            if (_mouseHookManager.Enabled) return;

            if (_config == null)
                _config = new ScreenConfig();

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
        private void ExitProg(object sender, EventArgs e) { Stop(); Shutdown(); }

        private void SystemEvents_DisplaySettingsChanged(object sender, EventArgs e)
        {
            _config = new ScreenConfig();
        }


        private void _MouseHookManager_MouseMoveExt(object sender, MouseEventExtArgs e)
        {

            if (_oldPoint == null)
            {
                _oldPoint = new PixelPoint(_config, null, e.X, e.Y);
                //                Debug.Print("New:" + (_oldPoint.Screen?.DeviceName??"null"));
                return;
            }

            if (e.Clicked) return;
            if (e.X == (int)_oldPoint.X && e.Y == (int)_oldPoint.Y) return;

            PixelPoint pIn = new PixelPoint(
                _oldPoint.Config,
                _oldPoint.Screen, e.X, e.Y);

//            _mouseLocation = pIn;


            if (pIn.TargetScreen == _oldPoint.Screen)
            {
                _oldPoint = pIn;
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
                Segment seg = new Segment(_oldPoint.Physical.Point, pIn.Physical.Point);

                Side side = Segment.OpositeSide(seg.IntersectSide(_oldPoint.Screen.PhysicalBounds));

                foreach (Screen s in _config.AllScreens)
                {
                    if (s == _oldPoint.Screen) continue;

                    foreach (
                        Point p in 
                        _config.AllowCornerCrossing?
                        seg.Line.Intersect(s.PhysicalBounds):
                        new Point[] {seg.Line.Intersect(s.PhysicalBounds, side)??new Point() }
                    )
                    {
                        Segment travel = new Segment(_oldPoint.Physical.Point, p);
                        if (!travel.Rect.Contains(pIn.Physical.Point)) continue;
                        if (travel.Size > dist) continue;

                        dist = travel.Size;
                        pIn = (new PhysicalPoint(_config, s, p.X, p.Y)).Pixel.Inside;
                        screenOut = s;
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
            //Mouse.CursorPos = pIn.Physical.ToScreen(screenOut).Pixel.Inside.Point;

            // Adjust pointer size to dpi ratio : should not be usefull if windows screen ratio is used
            // TODO : deactivate option when screen ratio exed 100%
            if (_config.AdjustPointer)
            {
                if (screenOut.RealDpiAvg > 110)
                {
                    Mouse.SetCursorAero(screenOut.RealDpiAvg > 138 ? 3 : 2);
                }
                else Mouse.SetCursorAero(1);
            }


            // Adjust pointer speed to dpi ratio : should not be usefull if windows screen ratio is used
            // TODO : deactivate option when screen ratio exed 100%
            if (_config.AdjustSpeed)
            {
                Mouse.MouseSpeed = Math.Round((5.0 / 96.0) * screenOut.RealDpiAvg, 0);
            }

            _oldPoint = new PixelPoint(_config, screenOut, pIn.X, pIn.Y);
            e.Handled = true;
        }

        public void Dispose()
        {
            //_notify?.Dispose();
        }

    }
}
