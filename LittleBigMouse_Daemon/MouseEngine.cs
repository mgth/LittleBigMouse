using System;
using System.Diagnostics;
using System.Globalization;
using System.Linq;
using System.ServiceModel;
using System.ServiceModel.Description;
using System.Windows;
using LbmScreenConfig;
using LittleBigMouseGeo;
using Microsoft.Win32;
using MonitorVcp;
using MouseKeyboardActivityMonitor;
using MouseKeyboardActivityMonitor.WinApi;

namespace LittleBigMouse_Daemon
{
    [ServiceBehavior(InstanceContextMode = InstanceContextMode.Single)]
    public class MouseEngine 
    {
        public ScreenConfig Config { get; private set; }
        private double _initMouseSpeed;
        public readonly MouseHookListener Hook = new MouseHookListener(new GlobalHooker());
        private PixelPoint _oldPoint = null;

        private ServiceHost _host;
        public void StartServer(ILittleBigMouseService service)
        {
            if (_host == null)
            {
                _host = new ServiceHost(service, LittleBigMouseClient.Address);
                ServiceMetadataBehavior smb = new ServiceMetadataBehavior
                {
                    MetadataExporter = { PolicyVersion = PolicyVersion.Policy15 }
                };

                _host.Description.Behaviors.Add(smb);
            }

            _host.Open();          
        }
        public void StopServer()
        {
            _host.Close();
        }

        public void Quit()
        {
            throw new NotImplementedException("MouseEngine : Quit");
        }

        public void Start()
        {
            if (Hook.Enabled) return;

            if (Config == null)
                LoadConfig();

            if (Config == null || !Config.Enabled) return;

            SystemEvents.DisplaySettingsChanged += OnDisplaySettingsChanged;

            using (RegistryKey key = ScreenConfig.OpenRootRegKey(true))
            {
                string ms = key.GetValue("InitialMouseSpeed", string.Empty).ToString();

                if (string.IsNullOrEmpty(ms))
                {
                    _initMouseSpeed = LbmMouse.MouseSpeed;
                    key.SetValue("InitialMouseSpeed", _initMouseSpeed.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String);
                }
                else 
                    double.TryParse(ms, out _initMouseSpeed);

                using (RegistryKey savekey = key.CreateSubKey("InitialCursor"))
                {
                    if (savekey?.ValueCount == 0)
                    {
                        LbmMouse.SaveCursor(savekey);
                    }
                }
            }

            Process.GetCurrentProcess().PriorityClass = ProcessPriorityClass.RealTime;

            Hook.MouseMoveExt += OnMouseMoveExt;
            Hook.Enabled = true;
 //           LittleBigMouseDaemon.Callback?.OnStateChange();
        }

        public void Stop()
        {
            SystemEvents.DisplaySettingsChanged -= OnDisplaySettingsChanged;

            if (!Hook.Enabled) return;

            Hook.MouseMoveExt -= OnMouseMoveExt;
            Hook.Enabled = false;
 //           LittleBigMouseDaemon.Callback?.OnStateChange();

            if (Config == null) return;

            if (Config.AdjustSpeed)
            {
                LbmMouse.MouseSpeed = _initMouseSpeed;
                using (var key = ScreenConfig.OpenRootRegKey(true))
                {
                    key.DeleteValue("InitialMouseSpeed");
                }
            }

            if (Config.AdjustPointer)
            {
                using (var key = ScreenConfig.OpenRootRegKey())
                {
                    using (RegistryKey savekey = key.OpenSubKey("InitialCursor"))
                    {
                        if (savekey != null)
                        {
                            LbmMouse.RestoreCursor(savekey);
                        }
                    }
                    key.DeleteSubKey("InitialCursor");
                }
            }

            Process.GetCurrentProcess().PriorityClass = ProcessPriorityClass.Normal;
        }


        public void LoadConfig(ScreenConfig config)
        {
            Config = config;
            ConfigLoaded?.Invoke(config,null);
        }

        public event EventHandler ConfigLoaded;
        public void LoadConfig()
        {
            LoadConfig(new ScreenConfig());
        }
        private void OnDisplaySettingsChanged(object sender, EventArgs e)
        {
            LoadConfig();
        }

        private void OnMouseMoveExt(object sender, MouseEventExtArgs e)
        {
            // If first time called just save that point
            if (_oldPoint == null)
            {
                _oldPoint = new PixelPoint(Config, null, e.X, e.Y);
                return;
            }


            if (e.Clicked) return;

            Screen oldScreen = _oldPoint.TargetScreen;

            PixelPoint pIn = new PixelPoint(
                _oldPoint.Config,
                oldScreen, e.X, e.Y);

            // No move
            if (pIn.Equals(_oldPoint)) return;

            //Debug.Print(pIn.X + " , " + pIn.Y + " -> " + pIn.TargetScreen?.Monitor.Adapter.DeviceName);

            // no screen change
            if (oldScreen == null || Equals(pIn.TargetScreen, oldScreen))
            {
                _oldPoint = pIn;
                return;
            }

            Screen screenOut = pIn.Mm.TargetScreen;

            Debug.Print("S>" + screenOut?.Monitor.Adapter.DeviceName);

            PixelPoint pOut = pIn;


            //
            // Allow Corner Jump
            //
            if (screenOut == null)
            {
                double dist = double.PositiveInfinity;// (100.0);
                Segment seg = new Segment(_oldPoint.Mm.Point, pIn.Mm.Point);

                // Calculate side to enter screen when corner crossing not allowed.
                Side side = seg.IntersectSide(_oldPoint.Screen.BoundsInMm);


                foreach (Screen screen in Config.AllScreens.Where(s => !Equals(s, oldScreen)))
                {
                    if (Config.AllowCornerCrossing)
                    {
                        foreach ( Point p in seg.Line.Intersect(screen.BoundsInMm) )
                        {
                            Segment travel = new Segment(_oldPoint.Mm.Point, p);
                            if (!travel.Rect.Contains(pIn.Mm.Point)) continue;
                            if (travel.Size > dist) continue;

                            dist = travel.Size;
                            pOut = (new PhysicalPoint(Config, screen, p.X, p.Y)).Pixel.Inside;
                            screenOut = screen;
                        }
                    }
                    else
                    {
                        Vector offset = new Vector(0,0);

                        switch (side)
                        {
                            case Side.None:
                                break;
                            case Side.Bottom:
                                offset.Y = seg.Rect.Height + screen.YLocationInMm - (oldScreen.YLocationInMm + oldScreen.HeightInMm);
                                if (offset.Y < 0) offset.Y = 0;
                                break;
                            case Side.Top:
                                offset.Y = -seg.Rect.Height + (screen.YLocationInMm + screen.HeightInMm) - oldScreen.YLocationInMm;
                                if (offset.Y > 0) offset.Y = 0;
                                break;
                            case Side.Right:
                                offset.X = seg.Rect.Width + screen.XLocationInMm - (oldScreen.XLocationInMm + oldScreen.WidthInMm);
                                if (offset.X < 0) offset.X = 0;
                                break;
                            case Side.Left:
                                offset.X = -seg.Rect.Width + (screen.XLocationInMm + screen.WidthInMm) - oldScreen.XLocationInMm;
                                if (offset.X > 0) offset.X = 0;
                                break;
                            default:
                                throw new ArgumentOutOfRangeException();
                        }

                        Debug.Print(screen.Monitor.Adapter.DeviceName + " = " + offset.Length);

                        if (offset.Length > 0 && offset.Length < dist)
                        {
                            Point shiftedPoint = pIn.Mm.Point + offset;
                            PhysicalPoint shifted = new PhysicalPoint(Config, screen, shiftedPoint.X , shiftedPoint.Y);
                            if (Equals(shifted.TargetScreen, screen))
                            {
                                dist = offset.Length;
                                pOut = shifted.Pixel;
                                screenOut = screen;                                                                                 
                            }
                            else
                            {
                                
                            }
                        }
                    }
                }
            }

            // if new position is not within another screen
            if (screenOut == null)
            {
                Debug.Print("Out");
                LbmMouse.CursorPos = pIn.Inside.Point;
                e.Handled = true;
                return;
            }


            // Actual mouving mouse to new location
            LbmMouse.CursorPos = pOut.Mm.ToScreen(screenOut).Pixel.Inside.Point;
            Debug.Print(">" + LbmMouse.CursorPos.X + "," + LbmMouse.CursorPos.Y);

            // Adjust pointer size to dpi ratio : should not be usefull if windows screen ratio is used
            if (Config.AdjustPointer)
            {
                if (screenOut.RealDpiAvg > 110)
                {
                    LbmMouse.SetCursorAero(screenOut.RealDpiAvg > 138 ? 3 : 2);
                }
                else LbmMouse.SetCursorAero(1);
            }


            // Adjust pointer speed to dpi ratio : should not be usefull if windows screen ratio is used
            if (Config.AdjustSpeed)
            {
                LbmMouse.MouseSpeed = Math.Round((5.0/96.0)*screenOut.RealDpiAvg, 0);
            }

            if (Config.HomeCinema)
            {
                oldScreen.Monitor.Vcp().Power = false;
            }
            screenOut.Monitor.Vcp().Power = true;

            _oldPoint = pOut;
            e.Handled = true;
        }

        public void MatchConfig(string configId)
        {
            Config.MatchConfig(configId);
        }
    }
}

