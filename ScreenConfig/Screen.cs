/*
  MouseControl - Mouse Managment in multi DPI monitors environment
  Copyright (c) 2015 Mathieu GRENET.  All right reserved.

  This file is part of MouseControl.

    ArduixPL is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    ArduixPL is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using Microsoft.Win32;
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Globalization;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows;
using System.Windows.Media;
using WinAPI_Dxva2;
using WinAPI_Gdi32;
using WinAPI_User32;
using Point = System.Windows.Point;
using Size = System.Windows.Size;

[assembly: InternalsVisibleTo("ScreenConfig")]
namespace LbmScreenConfig
{
    public class Screen : INotifyPropertyChanged
    {
        // PropertyChanged Handling
        public event PropertyChangedEventHandler PropertyChanged;
        private readonly PropertyChangeHandler _change;

        public IntPtr HMonitor { get; }
        internal Edid Edid;
        public ScreenConfig Config { get; }


        internal Screen(ScreenConfig config, IntPtr hMonitor)
        {
            _change = new PropertyChangeHandler(this);
            _change.PropertyChanged += delegate (object sender, PropertyChangedEventArgs args) { PropertyChanged?.Invoke(sender, args); };

            Config = config;
            HMonitor = hMonitor;

            MONITORINFOEX mi = new MONITORINFOEX();
            mi.Size = Marshal.SizeOf(mi);
            bool success = User32.GetMonitorInfo(hMonitor, ref mi);
            if (success)
            {
                DeviceName = mi.DeviceName;
                Primary = (mi.Flags==1);
                PixelBounds = GetRect(mi.Monitor);
                WorkingArea = GetRect(mi.WorkArea);

            }

            Edid = new Edid(this);


            PhysicalX = PhysicalOveralBoundsWithoutThis.Right;
        }

        ~Screen()
        {
            if (_pPhysicalMonitorArray != null && _pPhysicalMonitorArray.Length > 0)
                Dxva2.DestroyPhysicalMonitors((uint)_pPhysicalMonitorArray.Length, ref _pPhysicalMonitorArray);
        }


        // Todo, HPhysical not needed hes I think
        private PHYSICAL_MONITOR[] _pPhysicalMonitorArray;

        public IntPtr HPhysical
        {
            get
            {
                if (_pPhysicalMonitorArray!=null && _pPhysicalMonitorArray.Length>0) return _pPhysicalMonitorArray[0].hPhysicalMonitor;

                MONITORINFOEX monitorInfoEx = new MONITORINFOEX();
                monitorInfoEx.Size = (int)Marshal.SizeOf(monitorInfoEx);

                if (!User32.GetMonitorInfo(HMonitor, ref monitorInfoEx)) return IntPtr.Zero;

                uint pdwNumberOfPhysicalMonitors = 0;

                if (!Dxva2.GetNumberOfPhysicalMonitorsFromHMONITOR(HMonitor, ref pdwNumberOfPhysicalMonitors))
                    return IntPtr.Zero;

                // Récupère un handle physique du moniteur
                _pPhysicalMonitorArray = new PHYSICAL_MONITOR[pdwNumberOfPhysicalMonitors];

                if (Dxva2.GetPhysicalMonitorsFromHMONITOR(HMonitor, pdwNumberOfPhysicalMonitors, _pPhysicalMonitorArray))
                    return _pPhysicalMonitorArray[0].hPhysicalMonitor;
                //Nom = pPhysicalMonitorArray[0].szPhysicalMonitorDescription;

                _pPhysicalMonitorArray = null;
                uint err = WinAPI_Kernel32.Kernel32.GetLastError();

                return IntPtr.Zero;
            }
        }
        // References properties
        public string ProductCode => Edid.ProductCode;
        public string Model => Edid.Block((char)0xFC);
        public string ManufacturerCode => Edid.ManufacturerCode;
        public string PnpCode => ManufacturerCode + ProductCode;
        public string SerialNo => Edid.Block((char)0xFF);
        public string Serial => Edid.Serial;
        public string IdMonitor => PnpCode + "_" + Serial;

        [DependsOn("PixelSize")]
        public string IdResolution => PixelSize.Width + "x" + PixelSize.Height;
        [DependsOn("IdMonitor","IdResolution")]
        public string Id => IdMonitor + "_" + Orientation.ToString();
        public bool Primary { get; }
        public string DeviceName { get; }

        public string PnpDeviceName
        {
            get
            {
                if (_pnpName == null) _pnpName = Html.GetPnpName(PnpCode);
                return (_pnpName);
            }
        }

       public int Orientation
        {
            get
            {
                DEVMODE dev = new DEVMODE();

                User32.EnumDisplaySettings(DeviceName, -1, ref dev);

                return dev.DisplayOrientation;
            }
        }

        public int DeviceNo => int.Parse(DeviceName.Substring(11));

        // Physical dimensions
        // Natives
        private double _physicalX = double.NaN;
 
        public double PhysicalX {
            get { return double.IsNaN(_physicalX) ? 0 : _physicalX; }
            set
            {
                if (value == PhysicalX) return;

                if (Primary)
                {
                    foreach (Screen s in Config.AllScreens.Where(s => !s.Primary))
                    {
                        s.PhysicalX -= value;
                    }
                }
                else
                {
                    _change.SetProperty(ref _physicalX, value);
                }
            }
        }
        private double _physicalY = double.NaN;
        public double PhysicalY {
            get { return double.IsNaN(_physicalY) ? 0 : _physicalY; }
            set
            {
                if (value == PhysicalY) return;

                if (Primary)
                {
                    foreach (Screen s in Config.AllScreens.Where(s => !s.Primary))
                    {
                        s.PhysicalY -= value;
                    }
                }
                else
                {
                    _change.SetProperty(ref _physicalY, value);
                }
            }
        }

        private double _realPhysicalWidth = double.NaN;
        public double RealPhysicalWidth {
            get { return  double.IsNaN(_realPhysicalWidth) ? DeviceCapsPhysicalSize.Width : _realPhysicalWidth; }
            set
            {
                _change.SetProperty(ref _realPhysicalWidth, (value == DeviceCapsPhysicalSize.Width) ? double.NaN : value);
             }
        }

        private double _realPhysicalHeight = double.NaN;
        public double RealPhysicalHeight {
            get { return  double.IsNaN(_realPhysicalHeight)? DeviceCapsPhysicalSize.Height : _realPhysicalHeight; }
            set
            {
                _change.SetProperty(ref _realPhysicalHeight, (value==DeviceCapsPhysicalSize.Height)?double.NaN:value );
            }
        }

        [DependsOn("RealPhysicalWidth", "PixelSize")]
        public double RealPitchX
        {
            set { RealPhysicalWidth = PixelSize.Width * value; }
            get { return RealPhysicalWidth / PixelSize.Width; }
        }

        [DependsOn("RealPhysicalHeight", "PixelSize")]
        public double RealPitchY
        {
            set { RealPhysicalHeight = PixelSize.Height * value; }
            get { return RealPhysicalHeight / PixelSize.Height; }
        }

        private double _physicalRatioX = double.NaN;
        public double PhysicalRatioX
        {
            get { return double.IsNaN(_physicalRatioX) ? 1 : _physicalRatioX; }
            set { _change.SetProperty(ref _physicalRatioX, (value == 1) ? double.NaN : value); }
        }

        private double _physicalRatioY = double.NaN;
        public double PhysicalRatioY
        {
            get { return double.IsNaN(_physicalRatioY) ? 1 : _physicalRatioY; }
            set { _change.SetProperty(ref _physicalRatioY, (value == 1) ? double.NaN : value); }
        }

        //calculated
        [DependsOn("PhysicalRatioX", "RealPhysicalWidth")]
        public double PhysicalWidth => PhysicalRatioX * RealPhysicalWidth;

        [DependsOn("PhysicalRatioY", "RealPhysicalHeight")]
        public double PhysicalHeight => PhysicalRatioY * RealPhysicalHeight;

        [DependsOn("RealPitchX", "PhysicalRatioX")]
        public double PitchX => RealPitchX * PhysicalRatioX;
        [DependsOn("RealPitchY", "PhysicalRatioY")]
        public double PitchY => RealPitchY * PhysicalRatioY;

        [DependsOn("PhysicalX", "PhysicalY")]
        public Point PhysicalLocation
        {
            get
            {
                return new Point(PhysicalX, PhysicalY);
            }
            set
            {
                bool old = _change.Suspend();
                PhysicalX = value.X;
                PhysicalY = value.Y;
                _change.Resume(old);
            }
        }
        [DependsOn("PhysicalWidth", "PhysicalHeight")]
        public Size PhysicalSize => new Size(PhysicalWidth , PhysicalHeight);

        [DependsOn("PhysicalLocation", "PhysicalSize")]
        public Rect PhysicalBounds => new Rect(
            PhysicalLocation,
            PhysicalSize
            );

        [DependsOn("Config.PhysicalBounds")]
        public Rect PhysicalOveralBoundsWithoutThis
        {
            get
            {
                Rect r = new Rect();
                bool first = true;
                foreach (Screen s in Config.AllScreens.Where(s => s != this))
                {
                    if (first)
                    {
                        r = s.PhysicalBounds;
                        first = false;
                    }
                    else
                        r.Union(s.PhysicalBounds);
                }
                return r;
            }
        }

        // Pixel Dimensions native
        public Rect PixelBounds { get; }

        [DependsOn("PixelBounds")]
        public Size PixelSize => PixelBounds.Size;

        [DependsOn("PixelLocation", "PixelSize")]
        public AbsolutePoint BottomRight => new PixelPoint(Config, this, PixelLocation.X + PixelSize.Width, PixelLocation.Y + PixelSize.Height);

        [DependsOn("PixelBounds", "BottomRight")]
        public AbsoluteRectangle Bounds => new AbsoluteRectangle(new PixelPoint(Config, this, PixelBounds.X, PixelBounds.Y), BottomRight);

        [DependsOn("PixelBounds")]
        public PixelPoint PixelLocation => new PixelPoint(Config, this, PixelBounds.X, PixelBounds.Y);

        // Wpf
        [DependsOn("PixelBounds", "PixelToWpfRatioX")]
        public double WpfWidth => PixelBounds.Width*PixelToWpfRatioX;
        [DependsOn("PixelBounds", "PixelToWpfRatioY")]
        public double WpfHeight => PixelBounds.Height * PixelToWpfRatioY;



        // Registry settings

        public RegistryKey OpenMonitorRegKey(bool create=false)
        {
            using (RegistryKey key = Config.OpenRootRegKey(create))
            {
                if (key == null) return null;
                return create ?key.CreateSubKey(IdMonitor) :key.OpenSubKey(IdMonitor);
            }
        }

        public RegistryKey OpenGuiLocationRegKey(bool create = false)
        {
            using (RegistryKey key = OpenMonitorRegKey(create))
            {
                if (key == null) return null;
                return create ? key.CreateSubKey("GuiLocation") : key.OpenSubKey("GuiLocation");
            }
        }

        public RegistryKey OpenConfigRegKey(bool create = false)
        {
            using (RegistryKey key = Config.OpenConfigRegKey(create))
            {
                if (key == null) return null;
                return create ? key.CreateSubKey(IdMonitor) : key.OpenSubKey(IdMonitor);
            }
        }

        private void GetKeyDouble(ref double prop, RegistryKey key, string keyName, bool notify=true)
        {
            string sValue = key.GetValue(keyName, "NaN").ToString();
            if (sValue == "NaN") return;

            double value = double.Parse(sValue, CultureInfo.InvariantCulture);
            if (notify) _change.SetProperty(ref prop, value, keyName);
            else prop = value;
        }

        private static void SetKeyDouble(double prop, RegistryKey key, string keyName)
        {
            if (double.IsNaN(prop)) { key.DeleteValue(keyName, false); }
            else { key.SetValue(keyName, prop.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String); }
        }
        private void GetKeyString(ref string prop, RegistryKey key, string keyName)
        {
            string sValue = (string)key.GetValue(keyName, null);
            if (sValue == null) return;

            _change.SetProperty(ref prop, sValue, keyName);
        }

        private static void SetKeyString(ref string prop, RegistryKey key, string keyName)
        {
            if (prop == null) { key.DeleteValue(keyName, false); }
            else { key.SetValue(keyName, prop.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String); }
        }

        private string _pnpName;

        public void Load()
        {
            using (RegistryKey key = OpenGuiLocationRegKey())
            {
                if (key != null)
                {
                    double left = _guiLocation.Left, top = _guiLocation.Top, width = _guiLocation.Width, height = _guiLocation.Height;
                    GetKeyDouble(ref left, key, "Left", false);
                    GetKeyDouble(ref width, key, "Width", false);
                    GetKeyDouble(ref top, key, "Top", false);
                    GetKeyDouble(ref height, key, "Height", false);
                    _change.SetProperty(ref _guiLocation, new Rect(new Point(left, top), new Size(width, height)));
                }
            }

            using (RegistryKey key = OpenMonitorRegKey())
            {
                if (key != null)
                {
                    switch (Orientation)
                    {
                        case 0:
                            GetKeyDouble(ref _leftBorder, key, "LeftBorder");
                            GetKeyDouble(ref _rightBorder, key, "RightBorder");
                            GetKeyDouble(ref _topBorder, key, "TopBorder");
                            GetKeyDouble(ref _bottomBorder, key, "BottomBorder");
                            break;
                        case 1:
                            GetKeyDouble(ref _leftBorder, key, "TopBorder");
                            GetKeyDouble(ref _rightBorder, key, "BottomBorder");
                            GetKeyDouble(ref _topBorder, key, "LeftBorder");
                            GetKeyDouble(ref _bottomBorder, key, "RightBorder");
                            break;
                        case 2:
                            GetKeyDouble(ref _leftBorder, key, "RightBorder");
                            GetKeyDouble(ref _rightBorder, key, "LeftBorder");
                            GetKeyDouble(ref _topBorder, key, "BottomBorder");
                            GetKeyDouble(ref _bottomBorder, key, "TopBorder");
                            break;
                        case 3:
                            GetKeyDouble(ref _leftBorder, key, "BottomBorder");
                            GetKeyDouble(ref _rightBorder, key, "TopBorder");
                            GetKeyDouble(ref _topBorder, key, "RightBorder");
                            GetKeyDouble(ref _bottomBorder, key, "LeftBorder");
                            break;
                    }

                    switch (Orientation)
                    {
                        case 0:
                        case 2:
                            GetKeyDouble(ref _realPhysicalWidth, key, "PhysicalWidth");
                            GetKeyDouble(ref _realPhysicalHeight, key, "PhysicalHeight");
                            break;
                        case 1:
                        case 3:
                            GetKeyDouble(ref _realPhysicalWidth, key, "PhysicalHeight");
                            GetKeyDouble(ref _realPhysicalHeight, key, "PhysicalWidth");
                            break;
                    }
                    GetKeyString(ref _pnpName, key, "PnpName");
                }
            }

            using (RegistryKey key = OpenConfigRegKey())
            {
                if (key != null)
                {
                    GetKeyDouble(ref _physicalX, key, "PhysicalX");              
                    GetKeyDouble(ref _physicalY, key, "PhysicalY");
                    GetKeyDouble(ref _physicalRatioX, key, "PhysicalRatioX");
                    GetKeyDouble(ref _physicalRatioY, key, "PhysicalRatioY");
                }
            }
        }

        public void Save(RegistryKey baseKey)
        {
            using (RegistryKey key = OpenGuiLocationRegKey(true))
            {
                SetKeyDouble(_guiLocation.Left, key, "Left");
                SetKeyDouble(_guiLocation.Width, key, "Width");
                SetKeyDouble(_guiLocation.Top, key, "Top");
                SetKeyDouble(_guiLocation.Height, key, "Height");
            }
            using (RegistryKey key = OpenMonitorRegKey(true))
            {
                if (key != null)
                {
                    switch (Orientation)
                    {
                        case 0:
                            SetKeyDouble(_leftBorder, key, "LeftBorder");
                            SetKeyDouble(_rightBorder, key, "RightBorder");
                            SetKeyDouble(_topBorder, key, "TopBorder");
                            SetKeyDouble(_bottomBorder, key, "BottomBorder");
                            break;
                        case 1:
                            SetKeyDouble(_leftBorder, key, "TopBorder");
                            SetKeyDouble(_rightBorder, key, "BottomBorder");
                            SetKeyDouble(_topBorder, key, "LeftBorder");
                            SetKeyDouble(_bottomBorder, key, "RightBorder");
                            break;
                        case 2:
                            SetKeyDouble(_leftBorder, key, "RightBorder");
                            SetKeyDouble(_rightBorder, key, "LeftBorder");
                            SetKeyDouble(_topBorder, key, "BottomBorder");
                            SetKeyDouble(_bottomBorder, key, "TopBorder");
                            break;
                        case 3:
                            SetKeyDouble(_leftBorder, key, "BottomBorder");
                            SetKeyDouble(_rightBorder, key, "TopBorder");
                            SetKeyDouble(_topBorder, key, "RightBorder");
                            SetKeyDouble(_bottomBorder, key, "LeftBorder");
                            break;
                    }

                    switch (Orientation)
                    {
                        case 0:
                        case 2:
                            SetKeyDouble(_realPhysicalWidth, key, "PhysicalWidth");
                            SetKeyDouble(_realPhysicalHeight, key, "PhysicalHeight");
                            break;
                        case 1:
                        case 3:
                            SetKeyDouble(_realPhysicalWidth, key, "PhysicalHeight");
                            SetKeyDouble(_realPhysicalHeight, key, "PhysicalWidth");
                            break;
                    }
                    SetKeyString(ref _pnpName, key, "PnpName");
                }
            }

            using (RegistryKey key = OpenConfigRegKey(true))
            {
                if (key != null)
                {                    
                    SetKeyDouble(_physicalX, key, "PhysicalX");
                    SetKeyDouble(_physicalY, key, "PhysicalY");
                    SetKeyDouble(_physicalRatioX, key, "PhysicalRatioX");
                    SetKeyDouble(_physicalRatioY, key, "PhysicalRatioY");
                }
            }
        }



        public Rect WorkingArea { get; }

        public AbsoluteRectangle AbsoluteWorkingArea => new AbsoluteRectangle(
            new PixelPoint(Config,this,WorkingArea.X,WorkingArea.Y),
            new PixelPoint(Config,this,WorkingArea.Right, WorkingArea.Bottom) 
            );

        private static Rect GetRect(RECT value)
        {
            return new Rect
            {
                X = value.X,
                Y = value.Y,
                Width = value.Width,
                Height = value.Height
            };
        }

        public double RealLeftBorder
        {
            get { return _leftBorder; }
            set { _change.SetProperty(ref _leftBorder, value); }
        }
        public double RealRightBorder
        {
            get { return _rightBorder; }
            set { _change.SetProperty(ref _rightBorder, value); }
        }
        public double RealTopBorder
        {
            get { return _topBorder; }
            set { _change.SetProperty(ref _topBorder, value); }
        }
        public double RealBottomBorder
        {
            get { return _bottomBorder; }
            set { _change.SetProperty(ref _bottomBorder, value); }
        }

        [DependsOn("RealLeftBorder", "PhysicalRatioX")]
        public double LeftBorder => RealLeftBorder * PhysicalRatioX;

        [DependsOn("RealRightBorder", "PhysicalRatioX")]
        public double RightBorder => RealRightBorder * PhysicalRatioX;

        [DependsOn("RealTopBorder", "PhysicalRatioY")]
        public double TopBorder => RealTopBorder * PhysicalRatioY;

        [DependsOn("RealBottomBorder", "PhysicalRatioY")]
        public double BottomBorder => RealBottomBorder * PhysicalRatioY;

        [DependsOn("PhysicalX", "PhysicalY", "LeftBorder", "TopBorder", "RightBorder", "BottomBorder", "PhysicalWidth", "PhysicalHeight")]
        public Rect PhysicalOutsideBounds 
        {
            get
            {
                double x = PhysicalX - LeftBorder;
                double y = PhysicalY - TopBorder;
                double w = PhysicalWidth + LeftBorder + RightBorder;
                double h = PhysicalHeight + TopBorder + BottomBorder;
                return new Rect(new Point(x,y),new Size(w,h));
            }
        }

        private Rect _guiLocation;

        public Rect GuiLocation
        {
            get {
                if (_guiLocation.Width==0)
                {
                    Rect r = new Rect();
                    r.Width = 0.5;
                    r.Height = (9*WorkingArea.Width/16)/ WorkingArea.Height;
                    r.Y = 1 - r.Height;
                    r.X = 1 - r.Width;

                    return r;
                }
                    
                return _guiLocation;
            }
            set { _change.SetProperty(ref _guiLocation,value); }
        }

        public enum DpiType
        {
            Effective = 0,
            Angular = 1,
            Raw = 2,
        }        //https://msdn.microsoft.com/en-us/library/windows/desktop/dn280510.aspx
        [DllImport("Shcore.dll")]
        private static extern IntPtr GetDpiForMonitor([In]IntPtr hmonitor, [In]DpiType dpiType, [Out]out uint dpiX, [Out]out uint dpiY);

        private uint _winDpiX = 0;
        private uint _winDpiY = 0;
        private double _leftBorder = 20.0;
        private double _rightBorder = 20.0;
        private double _topBorder = 20.0;
        private double _bottomBorder = 20.0;

        private void GetWinDpi()
        {
            GetDpiForMonitor(HMonitor, DpiType.Effective, out _winDpiX, out _winDpiY);
        }
        public double WinDpiX
        {
            get
            {
                if (_winDpiX == 0) GetWinDpi();
                return _winDpiX;
            }
        }
        public double WinDpiY
        {
            get
            {
                if (_winDpiY == 0) GetWinDpi();
                return _winDpiY;
            }
        }

        public double ScaleFactor
        {
            get
            {
                int factor = 100;
                User32.GetScaleFactorForMonitor(HMonitor, ref factor);
                return (double)factor / 100;
            }
        }

        public Point ScaledPoint(Point p)
        {
            User32.POINT up = new User32.POINT((int)p.X, (int)p.Y);
            User32.PhysicalToLogicalPoint(HMonitor, ref up);
            return new Point(up.X, up.Y);
        }
        public double RawDpiX
        {
            get
            {
                uint dpiX;
                uint dpiY;
                GetDpiForMonitor(HMonitor, DpiType.Raw, out dpiX, out dpiY);
                return dpiX;
            }
        }
        public double RawDpiY
        {
            get
            {
                uint dpiX;
                uint dpiY;
                GetDpiForMonitor(HMonitor, DpiType.Raw, out dpiX, out dpiY);
                return dpiY;
            }
        }

        public double EffectiveDpiX
        {
            get
            {
                uint dpiX;
                uint dpiY;
                GetDpiForMonitor(HMonitor, DpiType.Effective, out dpiX, out dpiY);
                return dpiX;
            }
        }
        public double EffectiveDpiY
        {
            get
            {
                uint dpiX;
                uint dpiY;
                GetDpiForMonitor(HMonitor, DpiType.Effective, out dpiX, out dpiY);
                return dpiY;
            }
        }
        public double DpiAwareAngularDpiX
        {
            get
            {
                uint dpiX;
                uint dpiY;
                GetDpiForMonitor(HMonitor, DpiType.Angular, out dpiX, out dpiY);
                return dpiX;
            }
        }
        public double DpiAwareAngularDpiY
        {
            get
            {
                uint dpiX;
                uint dpiY;
                GetDpiForMonitor(HMonitor, DpiType.Angular, out dpiX, out dpiY);
                return dpiY;
            }
        }


        // This is the ratio used in system config
        [DependsOn("RealDpiX", "AngularDpiX", "EffectiveDpiX")]
        public double WpfToPixelRatioX => (Config.PrimaryScreen.EffectiveDpiX / 96) * Math.Round((RealDpiX / DpiAwareAngularDpiX) * 20) / 20;
        [DependsOn("RealDpiY", "AngularDpiY", "EffectiveDpiY")]
        public double WpfToPixelRatioY => (Config.PrimaryScreen.EffectiveDpiY / 96) * Math.Round((RealDpiY / DpiAwareAngularDpiY) * 20) / 20;

        [DependsOn("WpfToPixelRatioX")]
        public double PixelToWpfRatioX => 1 / WpfToPixelRatioX;
        [DependsOn("WpfToPixelRatioY")]
        public double PixelToWpfRatioY => 1 / WpfToPixelRatioY;

        [DependsOn("PitchX")]
        public double PhysicalToPixelRatioX => 1 / PitchX;
        [DependsOn("PitchY")]
        public double PhysicalToPixelRatioY => 1 / PitchY;


        [DependsOn("PhysicalToPixelRatioX", "WpfToPixelRatioX")]
        public double PhysicalToWpfRatioX => PhysicalToPixelRatioX / WpfToPixelRatioX;

        [DependsOn("PitchY")]
        public double PhysicalToWpfRatioY => PhysicalToPixelRatioY/ WpfToPixelRatioY;

        [DependsOn("RealPitchX")]
        public double RealDpiX
        {
            set { RealPitchX = 25.4 / value; }
            get { return 25.4 / RealPitchX; }
        }

        [DependsOn("RealPitchY")]
        public double RealDpiY
        {
            set { RealPitchY = 25.4 / value; }
            get { return 25.4 / RealPitchY; }
        }
        [DependsOn("PitchX")]
        public double DpiX => 25.4 / PitchX;

        [DependsOn("PitchY")]
        public double DpiY => 25.4 / PitchY;

        [DependsOn("PitchX","PitchY")]
        public double RealDpiAvg
        {
            get
            {
                double pitch = Math.Sqrt((PitchX * PitchX) + (PitchY * PitchY)) / Math.Sqrt(2);
                return 25.4 / pitch;
            }
        }

/*
        private double _graphicsDpiX = 0;
        private double GraphicsDpiX
        {
            get
            {
                if (_graphicsDpiX != 0) return _graphicsDpiX;

                IntPtr hdc = Gdi32.CreateDC(null, DeviceName, null, IntPtr.Zero);
                System.Drawing.Graphics gfx = System.Drawing.Graphics.FromHdc(hdc);
                _graphicsDpiX = gfx.DpiX;
                gfx.Dispose();
                Gdi32.DeleteDC(hdc);
                return _graphicsDpiX;
            }
        }

        private double _graphicsDpiY = 0;
        private double GraphicsDpiY
        {
            get
            {
                if (_graphicsDpiY != 0) return _graphicsDpiY;

                IntPtr hdc = Gdi32.CreateDC(null, DeviceName, null, IntPtr.Zero);
                System.Drawing.Graphics gfx = System.Drawing.Graphics.FromHdc(hdc);
                _graphicsDpiY = gfx.DpiY;
                gfx.Dispose();
                Gdi32.DeleteDC(hdc);
                return _graphicsDpiY;
            }
        }
        */

        private double LogPixelSx
        {
            get
            {
                IntPtr hdc = Gdi32.CreateDC("DISPLAY", DeviceName, null, IntPtr.Zero);
                double dpi = Gdi32.GetDeviceCaps(hdc, DeviceCap.LOGPIXELSX);
                Gdi32.DeleteDC(hdc);
                return dpi;
            }
        }
        private Size DeviceCapsPhysicalSize
        {
            get
            {
                IntPtr hdc = Gdi32.CreateDC("DISPLAY", DeviceName, null, IntPtr.Zero);
                double w = Gdi32.GetDeviceCaps(hdc, DeviceCap.HORZSIZE);
                double h = Gdi32.GetDeviceCaps(hdc, DeviceCap.VERTSIZE);
                Gdi32.DeleteDC(hdc);
                return new Size(w, h);
            }
        }



        public string CapabilitiesString
        {
            get
            {
                IntPtr hMonitor = HMonitor; //HPhysical;

                uint len = 0;
                if (!Gdi32.DDCCIGetCapabilitiesStringLength(hMonitor, ref len)) return "-1-";

                StringBuilder s = new StringBuilder((int)len + 1);

                if (!Gdi32.DDCCIGetCapabilitiesString(hMonitor, s, len)) return "-2-";

                return s.ToString();
            }
        }

        public bool Expand()
        {
            bool done = false;
            foreach (Screen s in Config.AllScreens)
            {
                if (s == this) continue;

                if (!PhysicalOutsideBounds.IntersectsWith(s.PhysicalOutsideBounds)) continue;

                double moveLeft = s.PhysicalOutsideBounds.X - (PhysicalOutsideBounds.X - s.PhysicalOutsideBounds.Width);
                double moveRight = (PhysicalOutsideBounds.X + PhysicalOutsideBounds.Width) - s.PhysicalOutsideBounds.X;
                double moveUp = s.PhysicalOutsideBounds.Y - (PhysicalOutsideBounds.Y - s.PhysicalOutsideBounds.Height);
                double moveDown = (PhysicalOutsideBounds.Y + PhysicalOutsideBounds.Height) - s.PhysicalOutsideBounds.Y;

                if(moveDown<=0 || moveLeft<=0 || moveUp<=0 || moveRight<=0) continue;

                done = true;
                if (moveLeft <= moveRight && moveLeft <= moveUp && moveLeft <= moveDown)
                {
                    s.PhysicalX -= moveLeft;
                }
                else if (moveRight <= moveLeft && moveRight <= moveUp && moveRight <= moveDown)
                {
                    s.PhysicalX += moveRight;

                }
                else if (moveUp <= moveRight && moveUp <= moveLeft && moveUp <= moveDown)
                {
                    s.PhysicalY -= moveUp;
                }
                else
                {
                    s.PhysicalY += moveDown;
                }
            }
            return done;

        }

        public bool PhysicalOverlapWith(Screen screen)
        {
            if (PhysicalX >= screen.PhysicalBounds.Right) return false;
            if (screen.PhysicalX >= PhysicalBounds.Right) return false;
            if (PhysicalY >= screen.PhysicalBounds.Bottom) return false;
            if (screen.PhysicalY >= PhysicalBounds.Bottom) return false;

            return true;
        }
        public bool PhysicalTouch(Screen screen)
        {
            if (PhysicalOverlapWith(screen)) return false;
            if (PhysicalX > screen.PhysicalBounds.Right) return false;
            if (screen.PhysicalX > PhysicalBounds.Right) return false;
            if (PhysicalY > screen.PhysicalBounds.Bottom) return false;
            if (screen.PhysicalY > PhysicalBounds.Bottom) return false;

            return true;
        }

        public double MoveLeftToTouch(Screen screen)
        {
            if (PhysicalY >= screen.PhysicalBounds.Bottom) return -1;
            if (screen.PhysicalY >= PhysicalBounds.Bottom) return -1;
            return PhysicalX - screen.PhysicalBounds.Right;
        }

        public double MoveRightToTouch(Screen screen)
        {
            if (PhysicalY >= screen.PhysicalBounds.Bottom) return -1;
            if (screen.PhysicalY >= PhysicalBounds.Bottom) return -1;
            return screen.PhysicalX - PhysicalBounds.Right;
        }
        public double MoveUpToTouch(Screen screen)
        {
            if (PhysicalX > screen.PhysicalBounds.Right) return -1;
            if (screen.PhysicalX > PhysicalBounds.Right) return -1;
            return PhysicalY - screen.PhysicalBounds.Bottom;
        }

        public double MoveDownToTouch(Screen screen)
        {
            if (PhysicalX > screen.PhysicalBounds.Right) return -1;
            if (screen.PhysicalX > PhysicalBounds.Right) return -1;
            return screen.PhysicalY - PhysicalBounds.Bottom;
        }


    }



}

