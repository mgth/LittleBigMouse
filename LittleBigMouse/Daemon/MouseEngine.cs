using System;
using System.Diagnostics;
using System.Globalization;
using System.Linq;
using System.ServiceModel;
using System.ServiceModel.Description;
using System.Windows;
using Hlab.Windows.MonitorVcp;
using HLab.Windows.Monitors;
using LbmScreenConfig;
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
        private Point? _oldPoint = null;

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
            MonitorsService.D.UpdateDevices();
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
                _oldPoint = new Point(e.X, e.Y);
                return;
            }


            if (e.Clicked) return;

            Screen oldScreen = Config.ScreenFromPixel(_oldPoint.Value);

            Point pIn = new Point(e.X, e.Y);

            // No move
            if (pIn.Equals(_oldPoint)) return;

            //Debug.Print(pIn.X + " , " + pIn.Y + " -> " + pIn.TargetScreen?.Monitor.Adapter.DeviceName);

            // no screen change
            if (oldScreen == null || Equals(Config.ScreenFromPixel(pIn), oldScreen))
            {
                _oldPoint = pIn;
                return;
            }

            Point oldpInMm = oldScreen.InMm.GetPoint(oldScreen.InPixel, _oldPoint.Value);
            Point pInMm = oldScreen.InMm.GetPoint(oldScreen.InPixel, pIn);
            Screen screenOut = null; //Config.ScreenFromMmPosition(pInMm);// pIn.Mm.TargetScreen;

            Debug.Print(oldScreen?.Monitor.Adapter.DeviceName + "P:" + _oldPoint +  " --> P:" + pIn + " " + screenOut?.Monitor.Adapter.DeviceName);

            Point pOut = pIn;


            //
            // Allow Corner Jump
            //
            if (screenOut == null)
            {
                double dist = double.PositiveInfinity;// (100.0);
                Segment seg = new Segment(oldpInMm, pInMm);

                // Calculate side to enter screen when corner crossing not allowed.
                Side side = seg.IntersectSide(oldScreen.InMm.Bounds);


                foreach (Screen screen in Config.AllScreens.Where(s => !Equals(s, oldScreen)))
                {
                    if (Config.AllowCornerCrossing)
                    {
                        foreach ( Point p in seg.Line.Intersect(screen.InMm.Bounds) )
                        {
                            Segment travel = new Segment(oldpInMm, p);
                            if (!travel.Rect.Contains(pInMm)) continue;
                            if (travel.Size > dist) continue;

                            dist = travel.Size;
                            pOut = screen.InPixel.GetPoint(screen.InMm, p);// (new PhysicalPoint(Config, screen, p.X, p.Y)).Pixel.Inside;
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
                                offset.Y = seg.Rect.Height + screen.InMm.Y - (oldScreen.InMm.Y + oldScreen.InMm.Height);
                                if (offset.Y < 0) offset.Y = 0;
                                break;
                            case Side.Top:
                                offset.Y = -seg.Rect.Height + (screen.InMm.Y + screen.InMm.Height) - oldScreen.InMm.Y;
                                if (offset.Y > 0) offset.Y = 0;
                                break;
                            case Side.Right:
                                offset.X = seg.Rect.Width + screen.InMm.X - (oldScreen.InMm.X + oldScreen.InMm.Width);
                                if (offset.X < 0) offset.X = 0;
                                break;
                            case Side.Left:
                                offset.X = -seg.Rect.Width + (screen.InMm.X + screen.InMm.Width) - oldScreen.InMm.X;
                                if (offset.X > 0) offset.X = 0;
                                break;
                            default:
                                throw new ArgumentOutOfRangeException();
                        }

                        Debug.Print(screen.Monitor.Adapter.DeviceName + " = " + offset.Length);

                        if (offset.Length > 0 && offset.Length < dist)
                        {
                            Point shiftedPoint = pInMm + offset;
                            
                            if (Equals(Config.ScreenFromMmPosition(shiftedPoint), screen))
                            {
                                dist = offset.Length;
                                pOut = screen.InPixel.GetPoint(screen.InMm,shiftedPoint);
                                pOut = screen.InPixel.Inside(pOut);
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
                LbmMouse.CursorPos = _oldPoint.Value;//TODO : .Inside.Point;
                e.Handled = true;
                return;
            }


            // Actual mouving mouse to new location
            LbmMouse.CursorPos = pOut;//.Mm.ToScreen(screenOut).Pixel.Inside.Point;
            Debug.Print(">" + pOut.X + "," + pOut.Y);
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

