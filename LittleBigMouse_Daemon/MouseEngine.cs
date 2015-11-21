using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Linq;
using System.ServiceModel;
using System.ServiceModel.Description;
using System.Windows;
using LbmScreenConfig;
using LittleBigMouseGeo;
using Microsoft.Win32;
using MouseKeyboardActivityMonitor;
using MouseKeyboardActivityMonitor.WinApi;

namespace LittleBigMouse_Daemon
{
    [ServiceBehavior(InstanceContextMode = InstanceContextMode.Single)]
    public class MouseEngine 
    {
        private ScreenConfig _config;
        private double _initMouseSpeed;
        private readonly MouseHookListener _hook = new MouseHookListener(new GlobalHooker());
        private readonly List<PixelPoint> _oldPoints = new List<PixelPoint>();

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
            throw new NotImplementedException();
        }

        public void Start()
        {
            if (_hook.Enabled) return;

            if (_config == null)
                LoadConfig();

            if (_config == null || !_config.Enabled) return;

            SystemEvents.DisplaySettingsChanged += OnDisplaySettingsChanged;

            using (RegistryKey key = ScreenConfig.OpenRootRegKey(true))
            {
                string ms = key.GetValue("InitialMouseSpeed", string.Empty).ToString();

                if (string.IsNullOrEmpty(ms))
                {
                    _initMouseSpeed = Mouse.MouseSpeed;
                    key.SetValue("InitialMouseSpeed", _initMouseSpeed.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String);
                }
                else 
                    double.TryParse(ms, out _initMouseSpeed);

                using (RegistryKey savekey = key.CreateSubKey("InitialCursor"))
                {
                    if (savekey?.ValueCount == 0)
                    {
                        Mouse.SaveCursor(savekey);
                    }
                }
            }

            _hook.MouseMoveExt += OnMouseMoveExt;
            _hook.Enabled = true;
        }

        public void Stop()
        {
            SystemEvents.DisplaySettingsChanged -= OnDisplaySettingsChanged;

            if (!_hook.Enabled) return;

            _hook.MouseMoveExt -= OnMouseMoveExt;
            _hook.Enabled = false;

            if (_config == null) return;

            if (_config.AdjustSpeed)
            {
                Mouse.MouseSpeed = _initMouseSpeed;
                using (var key = ScreenConfig.OpenRootRegKey(true))
                {
                    key.DeleteValue("InitialMouseSpeed");
                }
            }

            if (_config.AdjustPointer)
            {
                using (var key = ScreenConfig.OpenRootRegKey())
                {
                    using (RegistryKey savekey = key.OpenSubKey("InitialCursor"))
                    {
                        if (savekey != null)
                        {
                            Mouse.RestoreCursor(savekey);
                        }
                    }
                    key.DeleteSubKey("InitialCursor");
                }
            }
        }


        /// <summary>
        /// 
        /// </summary>
        /// <param name="p"></param>
        private void AddOldPoint(PixelPoint p)
        {
            _oldPoints.Add(p);
            if (_oldPoints.Count > 20) _oldPoints.Remove(_oldPoints.First());
        }
        public void LoadConfig(ScreenConfig config)
        {
            _config = config;
        }
        public void LoadConfig()
        {
            LoadConfig(new ScreenConfig());
        }
        private void OnDisplaySettingsChanged(object sender, EventArgs e)
        {
            LoadConfig();
        }

        public void Dump(PixelPoint pp)
        {
            foreach (var p in _oldPoints)
            {
                Debug.Print(p.X + " , " + p.Y);
            } 
            Debug.Print(pp.X + " , " + pp.Y);
            Debug.Print("---");
        }

        private void OnMouseMoveExt(object sender, MouseEventExtArgs e)
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

            Screen oldScreen = _oldPoints.Last().Screen;
            // no screen change
            if (pIn.TargetScreen == oldScreen)
            {
                AddOldPoint(pIn);
                return;
            }

            Screen screenOut = pIn.Physical.TargetScreen;
            PixelPoint pOut = pIn;

            //Debug.WriteLine("From:" + _oldPoint.Screen.DeviceName + " X:" + _oldPoint.X + " Y:" + _oldPoint.Y);
            //Debug.WriteLine(" To:" + (screenOut?.DeviceName ?? "null") + " X:" + pIn.X + " Y:" + pIn.Y + "\n");


            //
            // Allow Corner Jump
            //
            if (screenOut == null)
            {
                double dist = double.PositiveInfinity;// (100.0);
                Segment seg1 = new Segment(_oldPoints.First().Physical.Point, pIn.Physical.Point);
                Segment seg = new Segment(_oldPoints.Last().Physical.Point, pIn.Physical.Point);

                Debug.Print(seg.Line.Coef.ToString() + "(" + seg1.Line.Coef.ToString() + ")");
                Dump(pIn);

                // Calculate side to enter screen when corner crossing not allowed.
                Side side = seg.IntersectSide(_oldPoints.Last().Screen.PhysicalBounds);


                foreach (Screen screen in _config.AllScreens.Where(s => s != oldScreen))
                {
                    if (_config.AllowCornerCrossing)
                    {
                        foreach ( Point p in seg.Line.Intersect(screen.PhysicalBounds) )
                        {
                            Segment travel = new Segment(_oldPoints.Last().Physical.Point, p);
                            if (!travel.Rect.Contains(pIn.Physical.Point)) continue;
                            if (travel.Size > dist) continue;

                            dist = travel.Size;
                            pOut = (new PhysicalPoint(_config, screen, p.X, p.Y)).Pixel.Inside;
                            screenOut = screen;
                        }
                    }
                    else
                    {
                        double offsetX = 0;
                        double offsetY = 0;


                        switch (side)
                        {
                            case Side.None:
                                break;
                            case Side.Bottom:
                                offsetY = screen.PhysicalY - oldScreen.PhysicalY - oldScreen.PhysicalHeight;
                                if (offsetY < 0) offsetY = 0;
                                break;
                            case Side.Top:
                                offsetY = screen.PhysicalY + screen.PhysicalHeight - oldScreen.PhysicalY;
                                if (offsetY > 0) offsetY = 0;
                                break;
                            case Side.Right:
                                offsetX = screen.PhysicalX - oldScreen.PhysicalX - oldScreen.PhysicalWidth;
                                if (offsetX < 0) offsetX = 0;
                                break;
                            case Side.Left:
                                offsetX = screen.PhysicalX + screen.PhysicalWidth - oldScreen.PhysicalX;
                                if (offsetX > 0) offsetX = 0;
                                break;
                            default:
                                throw new ArgumentOutOfRangeException();
                        }

                        double length = Math.Sqrt(offsetX*offsetX + offsetY*offsetY);
                        Debug.Print(dist + " => " + screen.DeviceName + " = " + length);

                        if (length >0 && length < dist)
                        {
                            PhysicalPoint shifted = new PhysicalPoint(_config, screen, pIn.Physical.X + offsetX, pIn.Physical.Y + offsetY);
                            if (shifted.TargetScreen == screen)
                            {
                                dist = length;
                                pOut = shifted.Pixel;
                                screenOut = screen;              
                                                                      
                            }
                        }
                    }
                }
            }

            // if new position is not within another screen
            if (screenOut == null)
            {
                Mouse.CursorPos = pIn.Inside. /*MouseWpf.*/Point;
                e.Handled = true;
                return;
            }

            // Actual mouving mouse to new location
            Mouse.CursorPos = pOut.Physical.ToScreen(screenOut).Pixel. /*Inside.MouseWpf.*/Point;

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
                Mouse.MouseSpeed = Math.Round((5.0/96.0)*screenOut.RealDpiAvg, 0);
            }

            AddOldPoint(pOut);
            e.Handled = true;
        }

        public void MatchConfig(string configId)
        {
            _config.MatchConfig(configId);
        }
    }
}

