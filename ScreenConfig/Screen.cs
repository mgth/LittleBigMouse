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
using System.ComponentModel;
using System.Diagnostics;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Serialization;
using System.Text;
using System.Windows;
using WinAPI_Dxva2;
using WinAPI_Gdi32;
using WinAPI_User32;
using Point = System.Windows.Point;
using Size = System.Windows.Size;

[assembly: InternalsVisibleTo("ScreenConfig")]
namespace LbmScreenConfig
{
    [DataContract]
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
            Edid = new Edid(DeviceName);

            PhysicalX = PhysicalOveralBoundsWithoutThis.Right;
        }

        ~Screen()
        {
            if (_pPhysicalMonitorArray != null && _pPhysicalMonitorArray.Length > 0)
                Dxva2.DestroyPhysicalMonitors((uint)_pPhysicalMonitorArray.Length, ref _pPhysicalMonitorArray);
        }

        public string DeviceId
        {
            get
            {
                DISPLAY_DEVICE dd = Edid.DisplayDeviceFromId(DeviceName);
                return dd.DeviceID;
            }
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
                DISPLAY_DEVICE dd = Edid.DisplayDeviceFromId(DeviceName);
                string name = Html.CleanupPnpName(dd.DeviceString);
                if (name.ToLower() != "generic pnp monitor" )
                    return name;

                if (_pnpDeviceName != null)
                    return _pnpDeviceName;

                _pnpDeviceName = Html.GetPnpName(PnpCode);
                return _pnpDeviceName;
            }
        }

        [DependsOn("DeviceName")]
        public int DeviceNo => int.Parse(DeviceName.Substring(11));

       public int Orientation
        {
            get
            {
                DEVMODE dev = new DEVMODE();

                User32.EnumDisplaySettings(DeviceName, -1, ref dev);

                return dev.DisplayOrientation;
            }
        }


        // Physical dimensions
        // Natives
        private double _physicalX = double.NaN;
 
        //[DataMember]
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
        //[DataMember]
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
        //[DataMember]
        public double RealPhysicalWidth {
            get { return  double.IsNaN(_realPhysicalWidth) ? DeviceCapsPhysicalSize.Width : _realPhysicalWidth; }
            set
            {
                _change.SetProperty(ref _realPhysicalWidth, (value == DeviceCapsPhysicalSize.Width) ? double.NaN : value);
             }
        }

        private double _realPhysicalHeight = double.NaN;
        //[DataMember]
        public double RealPhysicalHeight {
            get { return  double.IsNaN(_realPhysicalHeight)? DeviceCapsPhysicalSize.Height : _realPhysicalHeight; }
            set
            {
                _change.SetProperty(ref _realPhysicalHeight, (value==DeviceCapsPhysicalSize.Height)?double.NaN:value );
            }
        }

        [DependsOn("RealPhysicalWidth", "PixelSize")]
       //[DataMember]
        public double RealPitchX
        {
            set { RealPhysicalWidth = PixelSize.Width * value; }
            get { return RealPhysicalWidth / PixelSize.Width; }
        }

        [DependsOn("RealPhysicalHeight", "PixelSize")]
        //[DataMember]
        public double RealPitchY
        {
            set { RealPhysicalHeight = PixelSize.Height * value; }
            get { return RealPhysicalHeight / PixelSize.Height; }
        }

        private double _physicalRatioX = double.NaN;
        //[DataMember]
        public double PhysicalRatioX
        {
            get { return double.IsNaN(_physicalRatioX) ? 1 : _physicalRatioX; }
            set { _change.SetProperty(ref _physicalRatioX, (value == 1) ? double.NaN : value); }
        }

        private double _physicalRatioY = double.NaN;
        //[DataMember]
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

        // Pixel native Dimensions 
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
        public double WpfWidth => PixelBounds.Width * PixelToWpfRatioX;
        [DependsOn("PixelBounds", "PixelToWpfRatioY")]
        public double WpfHeight => PixelBounds.Height * PixelToWpfRatioY;



        // Registry settings

        public static RegistryKey OpenMonitorRegKey(string id, bool create=false)
        {
            using (RegistryKey key = ScreenConfig.OpenRootRegKey(create))
            {
                if (key == null) return null;
                return create ?key.CreateSubKey(@"monitors\" + id) :key.OpenSubKey(@"monitors\" + id);
            }
        }

        public RegistryKey OpenMonitorRegKey(bool create = false)
        {
            return OpenMonitorRegKey(IdMonitor, create);
        }

        public RegistryKey OpenGuiLocationRegKey(bool create = false)
        {
            using (RegistryKey key = OpenMonitorRegKey(create))
            {
                if (key == null) return null;
                return create ? key.CreateSubKey("GuiLocation") : key.OpenSubKey("GuiLocation");
            }
        }

        public static RegistryKey OpenConfigRegKey(string configId, string monitorId, bool create = false)
        {
            using (RegistryKey key = ScreenConfig.OpenConfigRegKey(configId, create))
            {
                if (key == null) return null;
                return create ? key.CreateSubKey(monitorId) : key.OpenSubKey(monitorId);
            }
        }

        public RegistryKey OpenConfigRegKey(bool create = false)
        {
            return OpenConfigRegKey(Config.Id, IdMonitor, create);

            using (RegistryKey key = Config.OpenConfigRegKey(create))
            {
                if (key == null) return null;
                return create ? key.CreateSubKey(IdMonitor) : key.OpenSubKey(IdMonitor);
            }
        }

        private string _pnpDeviceName;

        public void Load()
        {
            using (RegistryKey key = OpenGuiLocationRegKey())
            {
                if (key != null)
                {
                    double left = _guiLocation.Left, top = _guiLocation.Top, width = _guiLocation.Width, height = _guiLocation.Height;
                    key.GetKey(ref left, "Left");
                    key.GetKey(ref width, "Width");
                    key.GetKey(ref top, "Top");
                    key.GetKey(ref height, "Height");
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
                            key.GetKey(ref _leftBorder,  "LeftBorder", _change);
                            key.GetKey(ref _rightBorder,  "RightBorder", _change);
                            key.GetKey(ref _topBorder, "TopBorder", _change);
                            key.GetKey(ref _bottomBorder, "BottomBorder", _change);
                            break;
                        case 1:
                            key.GetKey(ref _leftBorder, "TopBorder", _change);
                            key.GetKey(ref _rightBorder, "BottomBorder", _change);
                            key.GetKey(ref _topBorder, "LeftBorder", _change);
                            key.GetKey(ref _bottomBorder, "RightBorder", _change);
                            break;
                        case 2:
                            key.GetKey(ref _leftBorder, "RightBorder", _change);
                            key.GetKey(ref _rightBorder, "LeftBorder", _change);
                            key.GetKey(ref _topBorder, "BottomBorder", _change);
                            key.GetKey(ref _bottomBorder, "TopBorder", _change);
                            break;
                        case 3:
                            key.GetKey(ref _leftBorder, "BottomBorder", _change);
                            key.GetKey(ref _rightBorder, "TopBorder", _change);
                            key.GetKey(ref _topBorder, "RightBorder", _change);
                            key.GetKey(ref _bottomBorder, "LeftBorder", _change);
                            break;
                    }

                    switch (Orientation)
                    {
                        case 0:
                        case 2:
                            key.GetKey(ref _realPhysicalWidth, "PhysicalWidth", _change);
                            key.GetKey(ref _realPhysicalHeight, "PhysicalHeight", _change);
                            break;
                        case 1:
                        case 3:
                            key.GetKey(ref _realPhysicalWidth, "PhysicalHeight", _change);
                            key.GetKey(ref _realPhysicalHeight, "PhysicalWidth", _change);
                            break;
                    }
                    key.GetKey(ref _pnpDeviceName, "PnpName", _change);
                }
            }

            using (RegistryKey key = OpenConfigRegKey())
            {
                if (key != null)
                {
                    key.GetKey(ref _physicalX, "PhysicalX", _change);              
                    key.GetKey(ref _physicalY, "PhysicalY", _change);
                    key.GetKey(ref _physicalRatioX, "PhysicalRatioX", _change);
                    key.GetKey(ref _physicalRatioY, "PhysicalRatioY", _change);
                }
            }
        }

        public void Save(RegistryKey baseKey)
        {
            using (RegistryKey key = OpenGuiLocationRegKey(true))
            {
                key.SetKey(_guiLocation.Left, "Left");
                key.SetKey(_guiLocation.Width, "Width");
                key.SetKey(_guiLocation.Top, "Top");
                key.SetKey(_guiLocation.Height, "Height");
            }
            using (RegistryKey key = OpenMonitorRegKey(true))
            {
                if (key != null)
                {
                    switch (Orientation)
                    {
                        case 0:
                            key.SetKey(_leftBorder, "LeftBorder");
                            key.SetKey(_rightBorder, "RightBorder");
                            key.SetKey(_topBorder, "TopBorder");
                            key.SetKey(_bottomBorder, "BottomBorder");
                            break;
                        case 1:
                            key.SetKey(_leftBorder, "TopBorder");
                            key.SetKey(_rightBorder, "BottomBorder");
                            key.SetKey(_topBorder, "LeftBorder");
                            key.SetKey(_bottomBorder, "RightBorder");
                            break;
                        case 2:
                            key.SetKey(_leftBorder, "RightBorder");
                            key.SetKey(_rightBorder, "LeftBorder");
                            key.SetKey(_topBorder, "BottomBorder");
                            key.SetKey(_bottomBorder, "TopBorder");
                            break;
                        case 3:
                            key.SetKey(_leftBorder, "BottomBorder");
                            key.SetKey(_rightBorder, "TopBorder");
                            key.SetKey(_topBorder, "RightBorder");
                            key.SetKey(_bottomBorder, "LeftBorder");
                            break;
                    }

                    switch (Orientation)
                    {
                        case 0:
                        case 2:
                            key.SetKey(_realPhysicalWidth, "PhysicalWidth");
                            key.SetKey(_realPhysicalHeight, "PhysicalHeight");
                            break;
                        case 1:
                        case 3:
                            key.SetKey(_realPhysicalWidth, "PhysicalHeight");
                            key.SetKey(_realPhysicalHeight, "PhysicalWidth");
                            break;
                    }
                    key.SetKey(_pnpDeviceName, "PnpName");
                    key.SetKey(DeviceId,"DeviceId");
                }
            }

            using (RegistryKey key = OpenConfigRegKey(true))
            {
                if (key != null)
                {                    
                    key.SetKey(_physicalX, "PhysicalX");
                    key.SetKey(_physicalY, "PhysicalY");
                    key.SetKey(_physicalRatioX, "PhysicalRatioX");
                    key.SetKey(_physicalRatioY, "PhysicalRatioY");

                    key.SetKey(PixelLocation.X, "PixelX");
                    key.SetKey(PixelLocation.Y, "PixelY");
                    key.SetKey(PixelSize.Width, "PixelWidth");
                    key.SetKey(PixelSize.Height, "PixelHeight");

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

        private double _leftBorder = 20.0;
        public double RealLeftBorder
        {
            get { return _leftBorder; }
            set { _change.SetProperty(ref _leftBorder, value); }
        }

        private double _rightBorder = 20.0;
        public double RealRightBorder
        {
            get { return _rightBorder; }
            set { _change.SetProperty(ref _rightBorder, value); }
        }

        private double _topBorder = 20.0;
        public double RealTopBorder
        {
            get { return _topBorder; }
            set { _change.SetProperty(ref _topBorder, value); }
        }

        private double _bottomBorder = 20.0;
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


        private void GetWinDpi()
        {
            GetDpiForMonitor(HMonitor, DpiType.Effective, out _winDpiX, out _winDpiY);
        }

        private uint _winDpiX = 0;
        public double WinDpiX { get {
                if (_winDpiX == 0) GetWinDpi();
                return _winDpiX;
            }
        }

        private uint _winDpiY = 0;
        public double WinDpiY { get {
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

        private Process_DPI_Awareness DpiAwareness
        {
            get
            {
                    Process p = Process.GetCurrentProcess();

                    Process_DPI_Awareness aw = Process_DPI_Awareness.Process_Per_Monitor_DPI_Aware;

                    User32.GetProcessDpiAwareness(p.Handle, out aw);

                return aw;
            }
        }


        // This is the ratio used in system config
        [DependsOn("RealDpiX", "AngularDpiX", "EffectiveDpiX")]
        public double WpfToPixelRatioX
        {
            get
            {
                switch (DpiAwareness)
                {
                    case Process_DPI_Awareness.Process_DPI_Unaware:
                        return Math.Round((RealDpiX/DpiAwareAngularDpiX)*20)/20;
                    case Process_DPI_Awareness.Process_System_DPI_Aware:
                        return Config.PrimaryScreen.EffectiveDpiX / 96;
                    case Process_DPI_Awareness.Process_Per_Monitor_DPI_Aware:
                        return Config.PrimaryScreen.EffectiveDpiX / 96;//EffectiveDpiX/96;
                    default:
                        throw new ArgumentOutOfRangeException();
                }
            }
        }

        [DependsOn("RealDpiY", "AngularDpiY", "EffectiveDpiY")]
        public double WpfToPixelRatioY
        {
            get
            {
                switch (DpiAwareness)
                {
                    case Process_DPI_Awareness.Process_DPI_Unaware:
                        return Math.Round((RealDpiY / DpiAwareAngularDpiY) * 20) / 20;
                    case Process_DPI_Awareness.Process_System_DPI_Aware:
                        return Config.PrimaryScreen.EffectiveDpiY / 96;
                    case Process_DPI_Awareness.Process_Per_Monitor_DPI_Aware:
                        return Config.PrimaryScreen.EffectiveDpiY / 96;
                    default:
                        throw new ArgumentOutOfRangeException();
                }
            }
        }

        [DependsOn("WpfToPixelRatioX")]
        public double PixelToWpfRatioX => 1/WpfToPixelRatioX;

        [DependsOn("WpfToPixelRatioY")]
        public double PixelToWpfRatioY => 1/WpfToPixelRatioY;

        [DependsOn("PitchX")]
        public double PhysicalToPixelRatioX => 1/PitchX;

        [DependsOn("PitchY")]
        public double PhysicalToPixelRatioY => 1/PitchY;


        [DependsOn("PhysicalToPixelRatioX", "WpfToPixelRatioX")]
        public double PhysicalToWpfRatioX => PhysicalToPixelRatioX/WpfToPixelRatioX;

        [DependsOn("PitchY")]
        public double PhysicalToWpfRatioY => PhysicalToPixelRatioY/WpfToPixelRatioY;

        [DependsOn("RealPitchX")]
        public double RealDpiX
        {
            set { RealPitchX = 25.4/value; }
            get { return 25.4/RealPitchX; }
        }

        [DependsOn("RealPitchY")]
        public double RealDpiY
        {
            set { RealPitchY = 25.4/value; }
            get { return 25.4/RealPitchY; }
        }

        [DependsOn("PitchX")]
        public double DpiX => 25.4/PitchX;

        [DependsOn("PitchY")]
        public double DpiY => 25.4/PitchY;

        [DependsOn("PitchX", "PitchY")]
        public double RealDpiAvg
        {
            get
            {
                double pitch = Math.Sqrt((PitchX*PitchX) + (PitchY*PitchY))/Math.Sqrt(2);
                return 25.4/pitch;
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

                StringBuilder s = new StringBuilder((int) len + 1);

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

                if (moveDown <= 0 || moveLeft <= 0 || moveUp <= 0 || moveRight <= 0) continue;

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

        public static void AttachToDesktop(string configId, string monitorId)
        {
            string id;
            using (RegistryKey monkey = OpenMonitorRegKey(monitorId))
            {
               id = monkey?.GetValue("DeviceId").ToString();
                if (id == null) return;
            }

            double x = 0;
            double y = 0;
            double width = 0;
            double height = 0;

            using (RegistryKey monkey = OpenConfigRegKey(configId, monitorId))
            {
                x = double.Parse(monkey.GetValue("PixelX").ToString());
                y = double.Parse(monkey.GetValue("PixelY").ToString());
                width = double.Parse(monkey.GetValue("PixelWidth").ToString());
                height = double.Parse(monkey.GetValue("PixelHeight").ToString());
            }

            DISPLAY_DEVICE ddMon = Edid.DisplayDeviceFromDeviceId(id);
            DEVMODE devmode = new DEVMODE();
            devmode.Size = (short)Marshal.SizeOf(devmode);


            int idx = 0;
            while (true)
            {
                if (!User32.EnumDisplaySettings(ddMon.DeviceName, idx, ref devmode))
                return;

                if (devmode.PelsHeight == height && devmode.PelsWidth == width && devmode.BitsPerPel==32) break;
                idx++;
            }


            //devmode.Position = new POINTL { x = (int)x, y = (int)y };
            //devmode.Fields |= DM.Position;

            devmode.DeviceName = ddMon.DeviceName /*+ @"\Monitor0"*/;

            DISP_CHANGE ch = User32.ChangeDisplaySettingsEx(ddMon.DeviceName, ref devmode, IntPtr.Zero , ChangeDisplaySettingsFlags.CDS_UPDATEREGISTRY | ChangeDisplaySettingsFlags.CDS_NORESET, IntPtr.Zero);
            if (ch == DISP_CHANGE.Successful)
                ch = User32.ChangeDisplaySettingsEx(null, IntPtr.Zero, IntPtr.Zero, 0, IntPtr.Zero);
        }

        public void DetachFromDesktop()
        {
            DISPLAY_DEVICE ddMon = Edid.DisplayDeviceFromDeviceId(DeviceId);
            DEVMODE devmode = new DEVMODE();
            devmode.Size = (short)Marshal.SizeOf(devmode);

            devmode.DeviceName = ddMon.DeviceName ;
            devmode.PelsHeight = 0;
            devmode.PelsWidth = 0;
            devmode.Fields = DM.PelsWidth | DM.PelsHeight /*| DM.BitsPerPixel*/ | DM.Position
                        | DM.DisplayFrequency | DM.DisplayFlags ;

            DISP_CHANGE ch = User32.ChangeDisplaySettingsEx(ddMon.DeviceName, ref devmode, IntPtr.Zero, ChangeDisplaySettingsFlags.CDS_UPDATEREGISTRY | ChangeDisplaySettingsFlags.CDS_NORESET, IntPtr.Zero);
            if (ch == DISP_CHANGE.Successful)
                ch = User32.ChangeDisplaySettingsEx(null,IntPtr.Zero, IntPtr.Zero, 0,IntPtr.Zero);
        }
    }
}

