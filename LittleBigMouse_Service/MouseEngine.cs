using System;
using System.Threading;
using System.Windows;
using LbmScreenConfig;
using LittleBigMouseGeo;
using Microsoft.Win32;
using MouseKeyboardActivityMonitor;
using MouseKeyboardActivityMonitor.WinApi;

namespace LittleBigMouse_Service
{
    class MouseEngine
    {
        private ScreenConfig _config;

        private MouseHookListener _mouseHookManager;
        private PixelPoint _oldPoint;

        private Thread _keepAlive;

        public bool Start()
        {
            if (_mouseHookManager == null) _mouseHookManager = new MouseHookListener(new GlobalHooker());
            if (_mouseHookManager.Enabled) return false;

            if (_config == null)
                LoadConfig();

            if (_config == null) return false;
 //           if (!_config.Enabled) return false;

            SystemEvents.DisplaySettingsChanged += SystemEvents_DisplaySettingsChanged;
            _mouseHookManager.MouseMoveExt += _MouseHookManager_MouseMoveExt;
            _mouseHookManager.Enabled = true;

            _keepAlive = new Thread(loop);
            _keepAlive.Start();
            return true;
        }

        public void loop()
        {
            while (true) { Thread.Sleep(1000);}
        }

        public void Stop()
        {
            _keepAlive?.Abort();

            if (_mouseHookManager == null) return;
            if (!_mouseHookManager.Enabled)
            {
                _mouseHookManager.Dispose();
                _mouseHookManager = null;
                return;
            }

            SystemEvents.DisplaySettingsChanged -= SystemEvents_DisplaySettingsChanged;
            _mouseHookManager.MouseMoveExt -= _MouseHookManager_MouseMoveExt;
            _mouseHookManager.Enabled = false;
            _mouseHookManager.Dispose();
            _mouseHookManager = null;

            if (_config != null)
            {
                //todo : save initial configuration to be restored
                if (_config.AdjustSpeed)
                    Mouse.MouseSpeed = 10.0;

                if (_config.AdjustPointer)
                    Mouse.SetCursorAero(1);
            }
        }

        public void LoadConfig(ScreenConfig config)
        {
            _config = config;
        }


        private void SystemEvents_DisplaySettingsChanged(object sender, EventArgs e)
        {
            LoadConfig();
        }
        public void LoadConfig()
        {
            LoadConfig(new ScreenConfig());
        }


        private void _MouseHookManager_MouseMoveExt(object sender, MouseEventExtArgs e)
        {
            if (!_config.Enabled) return;

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
                        _config.AllowCornerCrossing ?
                        seg.Line.Intersect(s.PhysicalBounds) :
                        new Point[] { seg.Line.Intersect(s.PhysicalBounds, side) ?? new Point() }
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

    }
}

