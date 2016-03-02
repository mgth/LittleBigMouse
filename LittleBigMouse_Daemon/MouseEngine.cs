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
            throw new NotImplementedException();
        }

        public void Start()
        {
            if (Hook.Enabled) return;

            if (_config == null)
                LoadConfig();

            if (_config == null || !_config.Enabled) return;

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

            Hook.MouseMoveExt += OnMouseMoveExt;
            Hook.Enabled = true;
        }

        public void Stop()
        {
            SystemEvents.DisplaySettingsChanged -= OnDisplaySettingsChanged;

            if (!Hook.Enabled) return;

            Hook.MouseMoveExt -= OnMouseMoveExt;
            Hook.Enabled = false;

            if (_config == null) return;

            if (_config.AdjustSpeed)
            {
                LbmMouse.MouseSpeed = _initMouseSpeed;
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
                            LbmMouse.RestoreCursor(savekey);
                        }
                    }
                    key.DeleteSubKey("InitialCursor");
                }
            }
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
            Debug.Print("LoadConfig");
            LoadConfig();
        }

        private void OnMouseMoveExt(object sender, MouseEventExtArgs e)
        {
            // If first time called just save that point
            if (_oldPoint == null)
            {
                _oldPoint = new PixelPoint(_config, null, e.X, e.Y);
                return;
            }


            if (e.Clicked) return;

            Screen oldScreen = _oldPoint.TargetScreen;

            PixelPoint pIn = new PixelPoint(
                _oldPoint.Config,
                oldScreen, e.X, e.Y);

            // No move
            if (pIn.Equals(_oldPoint)) return;

            Debug.Print(pIn.X + " , " + pIn.Y + " -> " + pIn.TargetScreen?.DeviceName);

            // no screen change
            if (oldScreen == null || pIn.TargetScreen == oldScreen)
            {
                _oldPoint = pIn;
                return;
            }

            Screen screenOut = pIn.Physical.TargetScreen;

            Debug.Print("S>" + screenOut?.DeviceName);

            PixelPoint pOut = pIn;


            //
            // Allow Corner Jump
            //
            if (screenOut == null)
            {
                double dist = double.PositiveInfinity;// (100.0);
                Segment seg = new Segment(_oldPoint.Physical.Point, pIn.Physical.Point);

                // Calculate side to enter screen when corner crossing not allowed.
                Side side = seg.IntersectSide(_oldPoint.Screen.PhysicalBounds);


                foreach (Screen screen in _config.AllScreens.Where(s => s != oldScreen))
                {
                    if (_config.AllowCornerCrossing)
                    {
                        foreach ( Point p in seg.Line.Intersect(screen.PhysicalBounds) )
                        {
                            Segment travel = new Segment(_oldPoint.Physical.Point, p);
                            if (!travel.Rect.Contains(pIn.Physical.Point)) continue;
                            if (travel.Size > dist) continue;

                            dist = travel.Size;
                            pOut = (new PhysicalPoint(_config, screen, p.X, p.Y)).Pixel.Inside;
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
                                offset.Y = screen.PhysicalY - (oldScreen.PhysicalY + oldScreen.PhysicalHeight);
                                if (offset.Y < 0) offset.Y = 0;
                                break;
                            case Side.Top:
                                offset.Y = (screen.PhysicalY + screen.PhysicalHeight) - oldScreen.PhysicalY;
                                if (offset.Y > 0) offset.Y = 0;
                                break;
                            case Side.Right:
                                offset.X = screen.PhysicalX - (oldScreen.PhysicalX + oldScreen.PhysicalWidth);
                                if (offset.X < 0) offset.X = 0;
                                break;
                            case Side.Left:
                                offset.X = (screen.PhysicalX + screen.PhysicalWidth) - oldScreen.PhysicalX;
                                if (offset.X > 0) offset.X = 0;
                                break;
                            default:
                                throw new ArgumentOutOfRangeException();
                        }

                        Debug.Print(screen.DeviceName + " = " + offset.Length);

                        if (offset.Length > 0 && offset.Length < dist)
                        {
                            Point shiftedPoint = pIn.Physical.Point + offset;
                            PhysicalPoint shifted = new PhysicalPoint(_config, screen, shiftedPoint.X , shiftedPoint.Y);
                            if (shifted.TargetScreen == screen)
                            {
                                dist = offset.Length;
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
                Debug.Print("Out");
                LbmMouse.CursorPos = pIn.Inside.Point;
                e.Handled = true;
                return;
            }


            // Actual mouving mouse to new location
            LbmMouse.CursorPos = pOut.Physical.ToScreen(screenOut).Pixel.Point;
            Debug.Print(">" + LbmMouse.CursorPos.X + "," + LbmMouse.CursorPos.Y);

            // Adjust pointer size to dpi ratio : should not be usefull if windows screen ratio is used
            if (_config.AdjustPointer)
            {
                if (screenOut.RealDpiAvg > 110)
                {
                    LbmMouse.SetCursorAero(screenOut.RealDpiAvg > 138 ? 3 : 2);
                }
                else LbmMouse.SetCursorAero(1);
            }


            // Adjust pointer speed to dpi ratio : should not be usefull if windows screen ratio is used
            if (_config.AdjustSpeed)
            {
                LbmMouse.MouseSpeed = Math.Round((5.0/96.0)*screenOut.RealDpiAvg, 0);
            }

            if (_config.HomeCinema)
            {
                oldScreen.Monitor.Vcp().Power = false;
            }
            screenOut.Monitor.Vcp().Power = true;

            _oldPoint = pOut;
            e.Handled = true;
        }

        public void MatchConfig(string configId)
        {
            _config.MatchConfig(configId);
        }
    }
}

