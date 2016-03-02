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
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Serialization;
using System.Security.Cryptography.X509Certificates;
using System.Text;
using System.Windows;
using NotifyChange;
using WinAPI_Dxva2;
using WinAPI_Gdi32;
using WinAPI_User32;
using Point = System.Windows.Point;
using Size = System.Windows.Size;

[assembly: InternalsVisibleTo("ScreenConfig")]
namespace LbmScreenConfig
{
    [DataContract]
    public class Screen : Notifier
    {
        private Monitor _monitor;

        public Monitor Monitor
        {
            get { return _monitor; }
            private set { SetAndWatch(ref _monitor, value); }
        }
        public ScreenConfig Config { get; }

        public IEnumerable<Screen> OtherScreens => Config.AllScreens.Where(s => s != this);

        internal Screen(ScreenConfig config, Monitor monitor)
        {
            Config = config;
            Monitor = monitor;


            // Todo : PhysicalX = PhysicalOveralBoundsWithoutThis.Right;
        }

        ~Screen()
        {
        }

        //public string DeviceId
        //{
        //    get
        //    {
        //        DISPLAY_DEVICE dd = Edid.DisplayDeviceFromId(DeviceName);
        //        return dd.DeviceId;
        //    }
        //}



        // References properties
        public string ProductCode => Monitor.ProductCode;
        public string Model => Monitor.Model;
        public string ManufacturerCode => Monitor.ManufacturerCode;
        [DependsOn("ManufacturerCode", "ProductCode")]
        public string PnpCode => ManufacturerCode + ProductCode;
        public string SerialNo => Monitor.SerialNo;
        [DependsOn("Monitor.Serial")]
        public string Serial => Monitor.Serial;
        [DependsOn("PnpCode", "Serial")]
        public string IdMonitor => PnpCode + "_" + Serial;

        [DependsOn("PixelSize")]
        public string IdResolution => PixelSize.Width + "x" + PixelSize.Height;
        [DependsOn("IdMonitor", "Orientation")]
        public string Id => IdMonitor + "_" + Orientation.ToString();

        [DependsOn("Monitor.Flags")]
        public bool Primary => (Monitor.Flags == 1);


        public string DeviceName => Monitor.Adapter.DeviceName;

        public string PnpDeviceName
        {
            get
            {
                var name = Html.CleanupPnpName(Monitor.DeviceString);
                if (name.ToLower() != "generic pnp monitor")
                    return name;

                return _pnpDeviceName ?? (_pnpDeviceName = Html.GetPnpName(PnpCode));
            }
        }

        [DependsOn("DeviceName")]
        public int DeviceNo
        {
            get
            {
                var s = DeviceName.Split('\\');
                return s.Length < 4 ? 0 : int.Parse(s[3].Replace("DISPLAY", ""));
            }
        }

        private bool _selected;

        public bool Selected
        {
            get { return _selected; }
            set
            {
                if (!SetProperty(ref _selected, value)) return;
                if (!value) return;
                foreach (Screen screen in Config.AllBut(this)) screen.Selected = false;
            }
        }

        private bool _fixedAspectRatio = true;
        public bool FixedAspectRatio
        {
            get { return _fixedAspectRatio; }
            set { SetProperty(ref _fixedAspectRatio, value); }
        }

        [DependsOn("Monitor.DisplayOrientation")]
        public int Orientation => Monitor.DisplayOrientation;

        // Physical dimensions
        // Natives
        private double _physicalX = double.NaN;

        //[DataMember]
        public double PhysicalX
        {
            get { return double.IsNaN(_physicalX) ? 0 : _physicalX; }
            set { SetPhysicalX(value); }
        }

        public void SetPhysicalX(double value)
        {
            if (value == PhysicalX) return;

            if (Primary)
            {
                foreach (Screen s in Config.AllBut(this))
                {
                    s.SetPhysicalX(s.PhysicalX + _physicalX - value);
                }
                SetProperty(ref _physicalX, 0, nameof(PhysicalX));
            }
            else
            {
                SetProperty(ref _physicalX, value, nameof(PhysicalX));
            }
       }

        private double _physicalY = double.NaN;
        //[DataMember]
        public double PhysicalY
        {
            get { return double.IsNaN(_physicalY) ? 0 : _physicalY; }
            set { SetPhysicalY(value); }
        }

        public void SetPhysicalY(double value)
        {
            if (value == PhysicalY) return;

            if (Primary)
            {
                foreach (Screen s in Config.AllBut(this))
                {
                    s.SetPhysicalY(s.PhysicalY + _physicalY - value); //shift;
                }
                SetProperty(ref _physicalY, 0, nameof(PhysicalY));
            }
            else
            {
                SetProperty(ref _physicalY, value, nameof(PhysicalY));
            }
        }

        private double _realPhysicalWidth = double.NaN;
        //[DataMember]
        public double RealPhysicalWidth
        {
            get { return double.IsNaN(_realPhysicalWidth) ? Monitor.DeviceCapsHorzSize : _realPhysicalWidth; }
            set
            {
                double ratio = value / RealPhysicalWidth;
                SetRealPhysicalWidth(value);
                if (FixedAspectRatio) SetRealPhysicalHeight(RealPhysicalHeight * ratio);
            }
        }

        [DependsOn("Monitor.DeviceCapsHorzSize", "Monitor.DeviceCapsVertSize")]
        public void InitPhysicalSize()
        {
            if (double.IsNaN(_realPhysicalWidth)) SetRealPhysicalWidth(Monitor.DeviceCapsHorzSize);
            if (double.IsNaN(_realPhysicalHeight)) SetRealPhysicalHeight(Monitor.DeviceCapsVertSize);
        }

        private bool SetRealPhysicalWidth(double value)
        {
            return SetProperty(ref _realPhysicalWidth, value, nameof(RealPhysicalWidth));
        }


        private double _realPhysicalHeight = double.NaN;
        //[DataMember]
        public double RealPhysicalHeight
        {
            get { return double.IsNaN(_realPhysicalHeight) ? Monitor.DeviceCapsVertSize : _realPhysicalHeight; }
            set
            {
                double ratio = value / RealPhysicalHeight;
                SetRealPhysicalHeight(value);
                if (FixedAspectRatio) SetRealPhysicalWidth(RealPhysicalWidth * ratio);
            }
        }

        private bool SetRealPhysicalHeight(double value)
        {
            return SetProperty(ref _realPhysicalHeight, value, nameof(RealPhysicalHeight));
        }

        //public double[] AnchorsX => new[]
        //{
        //    PhysicalOutsideBounds.X,
        //    PhysicalX,
        //    PhysicalX + (PhysicalWidth/2),
        //    PhysicalBounds.Right,
        //    PhysicalOutsideBounds.Right,
        //};

        //public double[] AnchorsY => new[]
        //{
        //    PhysicalOutsideBounds.Y,
        //    PhysicalY,
        //    PhysicalY + (PhysicalHeight/2),
        //    PhysicalBounds.Bottom,
        //    PhysicalOutsideBounds.Bottom,
        //};


        [DependsOn(nameof(RealPhysicalWidth), nameof(PixelSize))]
        //[DataMember]
        public double RealPitchX
        {
            set { RealPhysicalWidth = PixelSize.Width * value; }
            get { return RealPhysicalWidth / PixelSize.Width; }
        }

        [DependsOn(nameof(RealPhysicalHeight), nameof(PixelSize))]
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
            set
            {
                SetProperty(ref _physicalRatioX, (value == 1) ? double.NaN : value);
            }
        }

        private double _physicalRatioY = double.NaN;
        //[DataMember]
        public double PhysicalRatioY
        {
            get { return double.IsNaN(_physicalRatioY) ? 1 : _physicalRatioY; }
            set
            {
                SetProperty(ref _physicalRatioY, (value == 1) ? double.NaN : value);
            }
        }

        //calculated
        [DependsOn(nameof(PhysicalRatioX), nameof(RealPhysicalWidth))]
        public double PhysicalWidth
            => PhysicalRatioX * RealPhysicalWidth;

        [DependsOn(nameof(PhysicalRatioY), nameof(RealPhysicalHeight))]
        public double PhysicalHeight => PhysicalRatioY * RealPhysicalHeight;

        [DependsOn(nameof(RealPitchX), nameof(PhysicalRatioX))]
        public double PitchX => RealPitchX * PhysicalRatioX;
        [DependsOn(nameof(RealPitchY), nameof(PhysicalRatioY))]
        public double PitchY => RealPitchY * PhysicalRatioY;

        [DependsOn(nameof(PhysicalX), nameof(PhysicalY))]
        public Point PhysicalLocation
        {
            get
            {
                return new Point(PhysicalX, PhysicalY);
            }
            set
            {
                PhysicalX = value.X;
                PhysicalY = value.Y;
            }
        }

        [DependsOn(nameof(PhysicalWidth), nameof(PhysicalHeight))]
        private void UpdatePhysicalSize()
        {
            PhysicalSize = new Size(PhysicalWidth, PhysicalHeight);
        }

        private Size _physicalSize = new Size(0, 0);
        public Size PhysicalSize
        {
            get
            {
                if (_physicalSize.Width == 0 || _physicalSize.Width == 0) throw new Exception("Not initialised");
                return _physicalSize;
            }
            private set { SetProperty(ref _physicalSize, value); }
        }

        [DependsOn(nameof(PhysicalLocation), nameof(PhysicalSize))]
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
                foreach (Screen s in Config.AllBut(this))
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
        [DependsOn("Monitor.MonitorArea")]
        public Rect PixelBounds => Monitor.MonitorArea;

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

        public static RegistryKey OpenMonitorRegKey(string id, bool create = false)
        {
            using (RegistryKey key = ScreenConfig.OpenRootRegKey(create))
            {
                if (key == null) return null;
                return create ? key.CreateSubKey(@"monitors\" + id) : key.OpenSubKey(@"monitors\" + id);
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

        public RegistryKey OpenConfigRegKey(bool create = false) => OpenConfigRegKey(Config.Id, IdMonitor, create);

        public string ConfigPath(bool create = false)
        {
            string path = Path.Combine(Config.ConfigPath(create), Id);
            if (create) System.IO.Directory.CreateDirectory(path);

            return path;
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
                    SetProperty(ref _guiLocation, new Rect(new Point(left, top), new Size(width, height)));
                }
            }

            using (RegistryKey key = OpenMonitorRegKey())
            {
                if (key != null)
                {
                    switch (Orientation)
                    {
                        case 0:
                            key.GetKey(ref _leftBorder, "LeftBorder", this);
                            key.GetKey(ref _rightBorder, "RightBorder", this);
                            key.GetKey(ref _topBorder, "TopBorder", this);
                            key.GetKey(ref _bottomBorder, "BottomBorder", this);
                            break;
                        case 1:
                            key.GetKey(ref _leftBorder, "TopBorder", this);
                            key.GetKey(ref _rightBorder, "BottomBorder", this);
                            key.GetKey(ref _topBorder, "LeftBorder", this);
                            key.GetKey(ref _bottomBorder, "RightBorder", this);
                            break;
                        case 2:
                            key.GetKey(ref _leftBorder, "RightBorder", this);
                            key.GetKey(ref _rightBorder, "LeftBorder", this);
                            key.GetKey(ref _topBorder, "BottomBorder", this);
                            key.GetKey(ref _bottomBorder, "TopBorder", this);
                            break;
                        case 3:
                            key.GetKey(ref _leftBorder, "BottomBorder", this);
                            key.GetKey(ref _rightBorder, "TopBorder", this);
                            key.GetKey(ref _topBorder, "RightBorder", this);
                            key.GetKey(ref _bottomBorder, "LeftBorder", this);
                            break;
                    }

                    switch (Orientation)
                    {
                        case 0:
                        case 2:
                            key.GetKey(ref _realPhysicalWidth, "PhysicalWidth", this);
                            key.GetKey(ref _realPhysicalHeight, "PhysicalHeight", this);
                            break;
                        case 1:
                        case 3:
                            key.GetKey(ref _realPhysicalWidth, "PhysicalHeight", this);
                            key.GetKey(ref _realPhysicalHeight, "PhysicalWidth", this);
                            break;
                    }
                    key.GetKey(ref _pnpDeviceName, "PnpName", this);
                }
            }

            using (RegistryKey key = OpenConfigRegKey())
            {
                if (key != null)
                {
                    key.GetKey(ref _physicalX, "PhysicalX", this);
                    key.GetKey(ref _physicalY, "PhysicalY", this);
                    key.GetKey(ref _physicalRatioX, "PhysicalRatioX", this);
                    key.GetKey(ref _physicalRatioY, "PhysicalRatioY", this);
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
                    key.SetKey(Monitor.DeviceId, "DeviceId");
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


        [DependsOn("Monitor.WorkArea")]
        public Rect WorkingArea => Monitor.WorkArea;

        public AbsoluteRectangle AbsoluteWorkingArea => new AbsoluteRectangle(
            new PixelPoint(Config, this, WorkingArea.X, WorkingArea.Y),
            new PixelPoint(Config, this, WorkingArea.Right, WorkingArea.Bottom)
            );


        private double _leftBorder = 20.0;
        public double RealLeftBorder
        {
            get { return _leftBorder; }
            set
            {
                if (value < 0) value = 0;
                SetProperty(ref _leftBorder, value);
            }
        }

        private double _rightBorder = 20.0;
        public double RealRightBorder
        {
            get { return _rightBorder; }
            set
            {
                if (value < 0) value = 0;
                SetProperty(ref _rightBorder, value);
            }
        }

        private double _topBorder = 20.0;
        public double RealTopBorder
        {
            get { return _topBorder; }
            set
            {
                if (value < 0) value = 0;
                SetProperty(ref _topBorder, value);
            }
        }

        private double _bottomBorder = 20.0;
        public double RealBottomBorder
        {
            get { return _bottomBorder; }
            set
            {
                if (value < 0) value = 0;
                SetProperty(ref _bottomBorder, value);
            }
        }

        [DependsOn(nameof(RealLeftBorder), nameof(PhysicalRatioX))]
        public double LeftBorder => RealLeftBorder * PhysicalRatioX;

        [DependsOn(nameof(RealRightBorder), nameof(PhysicalRatioX))]
        public double RightBorder => RealRightBorder * PhysicalRatioX;

        [DependsOn(nameof(RealTopBorder), nameof(PhysicalRatioY))]
        public double TopBorder => RealTopBorder * PhysicalRatioY;

        [DependsOn(nameof(RealBottomBorder), nameof(PhysicalRatioY))]
        public double BottomBorder => RealBottomBorder * PhysicalRatioY;


        private Rect _physicalOutsideBounds = new Rect(0, 0, 1, 1);

        public Rect PhysicalOutsideBounds => _physicalOutsideBounds;

        [DependsOn(nameof(PhysicalX), nameof(LeftBorder))]
        public void UpdatePhysicalOutsideBoundsX()
        {
            double x = PhysicalX - LeftBorder;
            if (x != _physicalOutsideBounds.X)
            {
                _physicalOutsideBounds.X = x;
                RaiseProperty(nameof(PhysicalOutsideBounds));
            }
        }
        [DependsOn(nameof(PhysicalY), nameof(TopBorder))]
        public void UpdatePhysicalOutsideBoundsY()
        {
            double y = PhysicalY - TopBorder;
            if (y != _physicalOutsideBounds.Y)
            {
                _physicalOutsideBounds.Y = y;
                RaiseProperty(nameof(PhysicalOutsideBounds));
            }
        }
        [DependsOn(nameof(PhysicalWidth), nameof(LeftBorder), nameof(RightBorder))]
        public void UpdatePhysicalOutsideBoundsWidth()
        {
            double w = PhysicalWidth + LeftBorder + RightBorder;
            if (w != _physicalOutsideBounds.Width)
            {
                _physicalOutsideBounds.Width = w;
                RaiseProperty(nameof(PhysicalOutsideBounds));
            }
        }
        [DependsOn(nameof(PhysicalHeight), nameof(TopBorder), nameof(BottomBorder))]
        public void UpdatePhysicalOutsideBoundsHeight()
        {
            double h = PhysicalHeight + TopBorder + BottomBorder;
            if (h != _physicalOutsideBounds.Height)
            {
                _physicalOutsideBounds.Height = h;
                RaiseProperty(nameof(PhysicalOutsideBounds));
            }
        }



        private Rect _guiLocation;
        public Rect GuiLocation
        {
            get
            {
                if (_guiLocation.Width == 0)
                {
                    Rect r = new Rect();
                    r.Width = 0.5;
                    r.Height = (9 * WorkingArea.Width / 16) / WorkingArea.Height;
                    r.Y = 1 - r.Height;
                    r.X = 1 - r.Width;

                    return r;
                }

                return _guiLocation;
            }
            set { SetProperty(ref _guiLocation, value); }
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
            GetDpiForMonitor(Monitor.HMonitor, DpiType.Effective, out _winDpiX, out _winDpiY);
        }

        private uint _winDpiX = 0;
        public double WinDpiX
        {
            get
            {
                if (_winDpiX == 0) GetWinDpi();
                return _winDpiX;
            }
        }

        private uint _winDpiY = 0;
        public double WinDpiY
        {
            get
            {
                if (_winDpiY == 0) GetWinDpi();
                return _winDpiY;
            }
        }

        public int SystemScaleFactor
        {
            get
            {
                int factor = 100;
                User32.GetScaleFactorForMonitor(Monitor.HMonitor, ref factor);
                return factor;
            }
        }
        public double ScaleFactor
        {
            get
            {
                switch (SystemScaleFactor)
                {
                    default:
                        return 1; //1.25
                    case 140:
                        return 1.5;//1.75
                    case 180:
                        return 2.00;//2.25 2.50 

                }
            }
        }

        public Point ScaledPoint(Point p)
        {
            User32.POINT up = new User32.POINT((int)p.X, (int)p.Y);
            User32.PhysicalToLogicalPoint(Monitor.HMonitor, ref up);
            return new Point(up.X, up.Y);
        }
        public double RawDpiX
        {
            get
            {
                uint dpiX;
                uint dpiY;
                GetDpiForMonitor(Monitor.HMonitor, DpiType.Raw, out dpiX, out dpiY);
                return dpiX;
            }
        }
        public double RawDpiY
        {
            get
            {
                uint dpiX;
                uint dpiY;
                GetDpiForMonitor(Monitor.HMonitor, DpiType.Raw, out dpiX, out dpiY);
                return dpiY;
            }
        }

        public double EffectiveDpiX
        {
            get
            {
                uint dpiX;
                uint dpiY;
                GetDpiForMonitor(Monitor.HMonitor, DpiType.Effective, out dpiX, out dpiY);
                return dpiX;
            }
        }
        public double EffectiveDpiY
        {
            get
            {
                uint dpiX;
                uint dpiY;
                GetDpiForMonitor(Monitor.HMonitor, DpiType.Effective, out dpiX, out dpiY);
                return dpiY;
            }
        }
        public double DpiAwareAngularDpiX
        {
            get
            {
                uint dpiX;
                uint dpiY;
                GetDpiForMonitor(Monitor.HMonitor, DpiType.Angular, out dpiX, out dpiY);
                return dpiX;
            }
        }
        public double DpiAwareAngularDpiY
        {
            get
            {
                uint dpiX;
                uint dpiY;
                GetDpiForMonitor(Monitor.HMonitor, DpiType.Angular, out dpiX, out dpiY);
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
                        return Math.Round((RealDpiX / DpiAwareAngularDpiX) * 20) / 20;
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
        public double PhysicalToWpfRatioY => PhysicalToPixelRatioY / WpfToPixelRatioY;

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

        [DependsOn("PitchX", "PitchY")]
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



        public string CapabilitiesString
        {
            get
            {
                IntPtr hMonitor = Monitor.HMonitor; //HPhysical;

                uint len = 0;
                if (!Gdi32.DDCCIGetCapabilitiesStringLength(hMonitor, ref len)) return "-1-";

                StringBuilder s = new StringBuilder((int)len + 1);

                if (!Gdi32.DDCCIGetCapabilitiesString(hMonitor, s, len)) return "-2-";

                return s.ToString();
            }
        }

        double RightDistance(Screen screen) =>  PhysicalOutsideBounds.X - screen.PhysicalOutsideBounds.Right;
        double LeftDistance(Screen screen) => screen.PhysicalOutsideBounds.X - PhysicalOutsideBounds.Right;
        double TopDistance(Screen screen) => screen.PhysicalOutsideBounds.Y - PhysicalOutsideBounds.Bottom;
        double BottomDistance(Screen screen) => PhysicalOutsideBounds.Y - screen.PhysicalOutsideBounds.Bottom;

        double RightDistanceToTouch(Screen screen, bool zero = false)
        {
            double top = TopDistance(screen);
            if ( top > 0 || (zero && top == 0) ) return double.PositiveInfinity;
            double bottom = BottomDistance(screen);
            if ( bottom > 0 || (zero && bottom ==0)) return double.PositiveInfinity;
            return RightDistance(screen);
        }

        double RightDistanceToTouch(IEnumerable<Screen> screens, bool zero = false)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(RightDistanceToTouch(screen,zero), dist);
            }
            return dist;
        }

        double RightDistance(IEnumerable<Screen> screens)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(RightDistance(screen), dist);
            }
            return dist;   
        }

        double LeftDistanceToTouch(Screen screen, bool zero = false)
        {
            double top = TopDistance(screen);
            if ( top > 0 || (zero && top==0)) return double.PositiveInfinity;
            double bottom = BottomDistance(screen);
            if ( bottom > 0 || (zero && bottom == 0)) return double.PositiveInfinity;
            return LeftDistance(screen);
        }
        double LeftDistanceToTouch(IEnumerable<Screen> screens, bool zero = false)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(LeftDistanceToTouch(screen,zero), dist);
            }
            return dist;
        }

        double LeftDistance(IEnumerable<Screen> screens)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(LeftDistance(screen), dist);
            }
            return dist;       
        }


        double TopDistanceToTouch(Screen screen, bool zero = false)
        {
            double left = LeftDistance(screen);
            if ( left > 0 || (zero && left==0)) return double.PositiveInfinity;
            double right = RightDistance(screen);
            if ( right > 0 || (zero && right==0)) return double.PositiveInfinity;
            return TopDistance(screen);
        }

        double TopDistanceToTouch(IEnumerable<Screen> screens, bool zero = false)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(TopDistanceToTouch(screen,zero), dist);
            }
            return dist;
        }

        double TopDistance(IEnumerable<Screen> screens)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(TopDistance(screen), dist);
            }
            return dist;           
        }

        double BottomDistanceToTouch(Screen screen, bool zero = false)
        {
            double left = LeftDistance(screen);
            if ( left > 0 || (zero && left==0)) return double.PositiveInfinity;
            double right = RightDistance(screen);
            if ( right > 0 || (zero && right==0)) return double.PositiveInfinity;
            return BottomDistance(screen);
        }

        double BottomDistanceToTouch(IEnumerable<Screen> screens, bool zero=false)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(BottomDistanceToTouch(screen, zero), dist);
            }
            return dist;
        }

        double BottomDistance(IEnumerable<Screen> screens)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(BottomDistance(screen), dist);
            }
            return dist;           
        }


        public double HorizontalDistance(Screen screen)
        {
            double right = RightDistance(screen);
            if (right >= 0) return right;

            double left = LeftDistance(screen);
            if (left >= 0) return left;

            return Math.Max(right, left);
        }

        public double HorizontalDistance(IEnumerable<Screen> screens)
        {
            double right = RightDistanceToTouch(screens);
            if (right >= 0) return right;

            double left = LeftDistanceToTouch(screens);
            if (left >= 0) return left;

            return Math.Max(right, left);
        }

        public double VerticalDistance(IEnumerable<Screen> screens)
        {
            double top = TopDistanceToTouch(screens);
            if (top >= 0) return top;

            double bottom = BottomDistanceToTouch(screens);
            if (bottom >= 0) return bottom;

            return Math.Max(top, bottom);
        }

        public double VerticalDistance(Screen screen)
        {
            double top = TopDistance(screen);
            if (top >= 0) return top;

            double bottom = BottomDistance(screen);
            if (bottom >= 0) return bottom;

            return Math.Max(top, bottom);
        }

        public double Distance(Screen screen)
        {
            Vector v = new Vector(HorizontalDistance(screen),VerticalDistance(screen));

             if (v.X >= 0 && v.Y >= 0) return v.Length;

            if (v.X >= 0) return v.X;
            if (v.Y >= 0) return v.Y;

            return Math.Max(v.X,v.Y);
        }

        public double Distance(IEnumerable<Screen> screens)
        {
            Vector v = new Vector(HorizontalDistance(screens), VerticalDistance(screens));

            if (v.X >= 0 && v.Y >= 0) return v.Length;

            if (v.X >= 0) return v.X;
            if (v.Y >= 0) return v.Y;

            return Math.Max(v.X, v.Y);
        }

        public bool Expand(Screen s)
        {
             double moveLeft = - LeftDistance(s); if (moveLeft <= 0) return false;

            double moveRight = - RightDistance(s); if (moveRight <= 0) return false;

            double moveUp = - TopDistance(s); if (moveUp <= 0) return false;

            double moveDown = - BottomDistance(s); if (moveDown <= 0) return false;

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
            
            return true;
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

            Adapter adapter = DisplayDevice.FromId(id);
            DEVMODE devmode = new DEVMODE();
            devmode.Size = (short)Marshal.SizeOf(devmode);


            int idx = 0;
            while (true)
            {
                if (!User32.EnumDisplaySettings(adapter.DeviceName, idx, ref devmode))
                    return;

                if (devmode.PelsHeight == height && devmode.PelsWidth == width && devmode.BitsPerPel == 32) break;
                idx++;
            }


            //devmode.Position = new POINTL { x = (int)x, y = (int)y };
            //devmode.Fields |= DM.Position;

            devmode.DeviceName = adapter.DeviceName /*+ @"\Monitor0"*/;

            DISP_CHANGE ch = User32.ChangeDisplaySettingsEx(adapter.DeviceName, ref devmode, IntPtr.Zero, ChangeDisplaySettingsFlags.CDS_UPDATEREGISTRY | ChangeDisplaySettingsFlags.CDS_NORESET, IntPtr.Zero);
            if (ch == DISP_CHANGE.Successful)
                ch = User32.ChangeDisplaySettingsEx(null, IntPtr.Zero, IntPtr.Zero, 0, IntPtr.Zero);
        }

        public void PlaceAuto(IEnumerable<Screen> screens)
        {
            double left = LeftDistanceToTouch(screens,true);
            double right = RightDistanceToTouch(screens, true);
            double top = TopDistanceToTouch(screens, true);
            double bottom = BottomDistanceToTouch(screens, true);

            if (!Config.AllowDiscontinuity && double.IsPositiveInfinity(left) && double.IsPositiveInfinity(top) && double.IsPositiveInfinity(right) && double.IsPositiveInfinity(bottom))
            {
                top = TopDistance(screens);
                right = RightDistance(screens);
                bottom = BottomDistance(screens);
                left = LeftDistance(screens);

                if (left > 0)
                {
                    if (top > 0)
                    {
                        PhysicalX += LeftDistance(screens);
                        PhysicalY += TopDistance(screens);                                          
                    }
                    if (bottom > 0)
                    {
                        PhysicalX += LeftDistance(screens);
                        PhysicalY -= BottomDistance(screens);
                    }
                }
                if (right > 0)
                {
                    if (top > 0)
                    {
                        PhysicalX -= RightDistance(screens);
                        PhysicalY += TopDistance(screens);
                    }
                    if (bottom > 0)
                    {
                        PhysicalX -= RightDistance(screens);
                        PhysicalY -= BottomDistance(screens);
                    }
                }

                left = LeftDistanceToTouch(screens,false);
                right = RightDistanceToTouch(screens, false);
                top = TopDistanceToTouch(screens, false);
                bottom = BottomDistanceToTouch(screens, false);
            }

            if (!Config.AllowDiscontinuity)
            {

                if (top > 0 && left > 0)
                {
                    if (left < top) PhysicalX += left;
                    else PhysicalY += top;
                    return;
                }

                if (top > 0 && right > 0)
                {
                    if (right < top) PhysicalX -= right;
                    else PhysicalY += top;
                    return;
                }

                if (bottom > 0 && right > 0)
                {
                    if (right < bottom) PhysicalX -= right;
                    else PhysicalY -= bottom;
                    return;
                }

                if (bottom > 0 && left > 0)
                {
                    if (left < bottom) PhysicalX += left;
                    else PhysicalY -= bottom;
                    return;
                }

                if (top < 0 && bottom < 0)
                {
                    if (left >= 0)
                    {
                        PhysicalX += left;
                        return;
                    }
                    if (right >= 0)
                    {
                        PhysicalX -= right;
                        return;
                    }
                }

                if (left < 0 && right < 0)
                {
                    //if (top >= 0)
                    if (top > 0)
                    {
                        PhysicalY += top;
                        return;
                    }
                    if (bottom >= 0)
                    {
                        PhysicalY -= bottom;
                        return;
                    }
                }
            }

            if (!Config.AllowOverlaps && left < 0 && right < 0 && top < 0 && bottom < 0)
            {
                if (left > right && left > top && left > bottom)
                {
                    PhysicalX += left;
                }
                else if (right > top && right > bottom)
                {
                    PhysicalX -= right;
                }
                else if (top > bottom)
                {
                    PhysicalY += top;
                }
                else PhysicalY -= bottom;
            }
        }
    }
}

