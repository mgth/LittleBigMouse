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
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Serialization;
using System.Text;
using System.Windows;
using WindowsMonitors;
using NotifyChange;
using WinAPI;

[assembly: InternalsVisibleTo("ScreenConfig")]

namespace LbmScreenConfig
{
    [DataContract]
    public class Screen : Notifier
    {
        public DisplayMonitor Monitor
        {
            get { return GetProperty<DisplayMonitor>(); }
            private set { SetAndWatch(value); }
        }

        public ScreenConfig Config { get; }

        public IEnumerable<Screen> OtherScreens => Config.AllScreens.Where(s => s != this);

        internal Screen(ScreenConfig config, DisplayMonitor monitor)
        {
            Config = config;
            Monitor = monitor;
            Watch(Config,"Config");

            // Todo : PhysicalX = PhysicalOveralBoundsWithoutThis.Right;
        }


        public override bool Equals(object obj)
        {
            Screen other = obj as Screen;
            if (obj==null) return base.Equals(obj);
            if (other == null) return false;
            return Monitor.Equals(other.Monitor);
        }

        public override int GetHashCode()
        {
            return ("Screen" + Monitor.DeviceId).GetHashCode();
        }

        ~Screen()
        {
        }


        // References properties
        [DependsOn("Monitor.ManufacturerCode", "Monitor.ProductCode")]
        public string PnpCode => Monitor.ManufacturerCode + Monitor.ProductCode;

        [DependsOn(nameof(PnpCode), "Monitor.Serial")]
        public string IdMonitor => PnpCode + "_" + Monitor.Serial;

        [DependsOn(nameof(PixelSize))]
        public string IdResolution => PixelSize.Width + "x" + PixelSize.Height;

        [DependsOn(nameof(IdMonitor), "Monitor.DisplayOrientation")]
        public string Id => IdMonitor + "_" + Monitor.DisplayOrientation.ToString();

        [DependsOn("Monitor.Primary")]
        public bool Primary => (Monitor.Primary == 1);


        public string PnpDeviceName => GetProperty<string>();

        public string PnpDeviceName_default
        {
            get
            {
                var name = Html.CleanupPnpName(Monitor.DeviceString);
                using (RegistryKey key = OpenConfigRegKey())
                {
                    name = key.GetKey("PnpName", name);
                }

                if (name.ToLower() != "generic pnp monitor")
                    return name;

                return Html.GetPnpName(PnpCode);
            }
        }

        [DependsOn("Monitor.DeviceName")]
        public int DeviceNo
        {
            get
            {
                var s = Monitor.DeviceName.Split('\\');
                return s.Length < 4 ? 0 : int.Parse(s[3].Replace("DISPLAY", ""));
            }
        }

        public bool Selected
        {
            get { return GetProperty<bool>(); }
            set
            {
                if (!SetProperty(value)) return;
                if (!value) return;
                foreach (Screen screen in Config.AllBut(this)) screen.Selected = false;
            }
        }

        public bool FixedAspectRatio
        {
            get { return GetProperty<bool>(); }
            set { SetProperty(value); }
        }

        // Physical dimensions
        // Natives

        [DataMember]
        public double PhysicalX
        {
            get
            {
                var value = GetProperty<double>();
                return double.IsNaN(value) ? 0 : value;
            }
            set
            {
                if (SetPhysicalX(value))
                    Config.Saved = false;
            }
        }

        public double PhysicalX_default
        {
            get
            {
                using (RegistryKey key = OpenConfigRegKey())
                {
                    return key.GetKey(nameof(PhysicalX), double.NaN);
                }
            }
        }



        public bool SetPhysicalX(double value)
        {
            if (value == PhysicalX) return false;

            if (Primary)
            {
                foreach (Screen s in Config.AllBut(this))
                {
                    s.SetPhysicalX(s.PhysicalX + PhysicalX - value);
                }
                return SetProperty(0.0, nameof(PhysicalX));
            }
            return SetProperty(value, nameof(PhysicalX));
        }

        [DataMember]
        public double PhysicalY
        {
            get
            {
                var value = GetProperty<double>();
                return double.IsNaN(value) ? 0 : value;
            }
            set
            {
                if (SetPhysicalY(value))
                    Config.Saved = false;
            }
        }

        public bool SetPhysicalY(double value)
        {
            if (value == PhysicalY) return false;

            if (Primary)
            {
                foreach (Screen s in Config.AllBut(this))
                {
                    s.SetPhysicalY(s.PhysicalY + PhysicalY - value); //shift;
                }
                return SetProperty(0.0, nameof(PhysicalY));
            }
            return SetProperty(value, nameof(PhysicalY));
        }
        public double PhysicalY_default
        {
            get
            {
                using (RegistryKey key = OpenConfigRegKey())
                {
                    return key.GetKey(nameof(PhysicalY), double.NaN);
                }
            }
        }

        [DataMember]

        [DependsOn("Monitor.DeviceCapsHorzSize")]
        public double RealPhysicalWidth
        {
            get
            {
                var value = GetProperty<double>();
                return double.IsNaN(value) ? (Monitor?.DeviceCapsHorzSize ?? 0) : value;
            }
            set
            {
                double ratio = value/RealPhysicalWidth;
                if (SetRealPhysicalWidth(value))
                    Config.Saved = false;
                if (FixedAspectRatio) SetRealPhysicalHeight(RealPhysicalHeight*ratio);
            }
        }

        public double RealPhysicalWidth_default => LoadRotatedValue(DimNames,"Physical%","Width", double.NaN);

        public double UnrotatedRealPhysicalWidth
        {
            get { return (Monitor.DisplayOrientation%2 == 0) ? RealPhysicalWidth : RealPhysicalHeight; }
            set
            {
                double ratio = value/UnrotatedRealPhysicalWidth;
                if (SetRealPhysicalWidth(value) && FixedAspectRatio)
                    SetUnrotatedRealPhysicalHeight(UnrotatedRealPhysicalHeight*ratio);
            }
        }

        private bool SetUnrotatedRealPhysicalWidth(double value)
        {
            switch (Monitor.DisplayOrientation%2)
            {
                case 0:
                    return SetRealPhysicalWidth(value);
                case 1:
                    return SetRealPhysicalHeight(value);
            }
            return false;
        }

        public double UnrotatedRealPhysicalHeight
        {
            get { return (Monitor.DisplayOrientation%2 == 0) ? RealPhysicalHeight : RealPhysicalWidth; }
            set
            {
                double ratio = value/UnrotatedRealPhysicalHeight;
                if (SetUnrotatedRealPhysicalHeight(value) && FixedAspectRatio)
                    SetUnrotatedRealPhysicalWidth(UnrotatedRealPhysicalWidth*ratio);
            }
        }

        private bool SetUnrotatedRealPhysicalHeight(double value)
        {
            switch (Monitor.DisplayOrientation%2)
            {
                case 0:
                    return SetRealPhysicalHeight(value);
                case 1:
                    return SetRealPhysicalWidth(value);
            }
            return false;

        }

        [DependsOn("Monitor.DeviceCapsHorzSize", "Monitor.DeviceCapsVertSize")]
        public void InitPhysicalSize()
        {
            if (Monitor == null) return;
            if (double.IsNaN(GetProperty<double>(nameof(RealPhysicalWidth))))
                SetRealPhysicalWidth(Monitor.DeviceCapsHorzSize);
            if (double.IsNaN(GetProperty<double>(nameof(RealPhysicalHeight))))
                SetRealPhysicalHeight(Monitor.DeviceCapsVertSize);
        }

        private bool SetRealPhysicalWidth(double value)
        {
            return SetProperty(value, nameof(RealPhysicalWidth));
        }


        [DataMember]
        [DependsOn("Monitor.DeviceCapsVertSize")]
        public double RealPhysicalHeight
        {
            get
            {
                double rph = GetProperty<double>();
                return double.IsNaN(rph) ? (Monitor?.DeviceCapsVertSize ?? 0) : rph;
            }
            set
            {
                double ratio = value/RealPhysicalHeight;
                if (SetRealPhysicalHeight(value))
                    Config.Saved = false;
                if (FixedAspectRatio) SetRealPhysicalWidth(RealPhysicalWidth*ratio);
            }
        }

        public double RealPhysicalHeight_default => LoadRotatedValue(DimNames, "Physical%", "Height", double.NaN);

        private bool SetRealPhysicalHeight(double value) => SetProperty(value, nameof(RealPhysicalHeight));

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
            set { RealPhysicalWidth = PixelSize.Width*value; }
            get { return RealPhysicalWidth/PixelSize.Width; }
        }

        [DependsOn(nameof(RealPhysicalHeight), nameof(PixelSize))]
        //[DataMember]
        public double RealPitchY
        {
            set { RealPhysicalHeight = PixelSize.Height*value; }
            get { return RealPhysicalHeight/PixelSize.Height; }
        }

        //[DataMember]
        public double PhysicalRatioX
        {
            get
            {
                var value = GetProperty<double>();
                return double.IsNaN(value) ? 1 : value;
            }
            set { SetProperty((value == 1) ? double.NaN : value); }
        }
        public double PhysicalRatioX_default => double.NaN;

        //[DataMember]
        public double PhysicalRatioY
        {
            get
            {
                var value = GetProperty<double>();
                return double.IsNaN(value) ? 1 : value;
            }
            set { SetProperty((value == 1) ? double.NaN : value); }
        }
        public double PhysicalRatioY_default => double.NaN;

        //calculated
        [DependsOn(nameof(PhysicalRatioX), nameof(RealPhysicalWidth))]
        public double PhysicalWidth
            => PhysicalRatioX*RealPhysicalWidth;

        [DependsOn(nameof(PhysicalRatioY), nameof(RealPhysicalHeight))]
        public double PhysicalHeight => PhysicalRatioY*RealPhysicalHeight;

        [DependsOn(nameof(RealPitchX), nameof(PhysicalRatioX))]
        public double PitchX => RealPitchX*PhysicalRatioX;

        [DependsOn(nameof(RealPitchY), nameof(PhysicalRatioY))]
        public double PitchY => RealPitchY*PhysicalRatioY;

        [DependsOn(nameof(PhysicalX), nameof(PhysicalY))]
        public Point PhysicalLocation
        {
            get { return new Point(PhysicalX, PhysicalY); }
            set
            {
                PhysicalX = value.X;
                PhysicalY = value.Y;
            }
        }

        [DependsOn(nameof(Monitor), nameof(PhysicalWidth), nameof(PhysicalHeight))]
        private void UpdatePhysicalSize()
        {
            PhysicalSize = new Size(PhysicalWidth, PhysicalHeight);
        }

        public Size PhysicalSize
        {
            get
            {
                //if (_physicalSize.Width == 0 || _physicalSize.Width == 0) throw new Exception("Not initialised");
                return GetProperty<Size>();
            }
            private set { SetProperty(value); }
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

        [DependsOn(nameof(PixelBounds))]
        public Size PixelSize => PixelBounds.Size;

        [DependsOn(nameof(PixelLocation), nameof(PixelSize))]
        public AbsolutePoint BottomRight
            => new PixelPoint(Config, this, PixelLocation.X + PixelSize.Width, PixelLocation.Y + PixelSize.Height);

        [DependsOn(nameof(PixelBounds), nameof(BottomRight))]
        public AbsoluteRectangle Bounds
            => new AbsoluteRectangle(new PixelPoint(Config, this, PixelBounds.X, PixelBounds.Y), BottomRight);

        [DependsOn(nameof(PixelBounds))]
        public PixelPoint PixelLocation => new PixelPoint(Config, this, PixelBounds.X, PixelBounds.Y);

        // Wpf
        [DependsOn(nameof(PixelBounds), nameof(PixelToWpfRatioX))]
        public double WpfWidth => PixelBounds.Width*PixelToWpfRatioX;

        [DependsOn(nameof(PixelBounds), nameof(PixelToWpfRatioY))]
        public double WpfHeight => PixelBounds.Height*PixelToWpfRatioY;



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

        [DependsOn("Config.Id")]
        void UpdateConfigId()
        {
            PhysicalX = PhysicalX_default;
            PhysicalY = PhysicalY_default;
            PhysicalRatioX = PhysicalRatioX_default;
            PhysicalRatioY = PhysicalRatioY_default;
        }

        [DependsOn("Id")]
        void UpdateId()
        {
            RealLeftBorder = RealLeftBorder_default;
            RealTopBorder = RealTopBorder_default;
            RealRightBorder = RealRightBorder_default;
            RealBottomBorder = RealBottomBorder_default;

            RealPhysicalWidth = RealPhysicalWidth_default;
            RealPhysicalHeight = RealPhysicalHeight_default;
        }


        public void Load()
        {
            using (RegistryKey key = OpenGuiLocationRegKey())
            {
                if (key != null)
                {
                    double left = GuiLocation.Left,
                    top = GuiLocation.Top,
                    width = GuiLocation.Width,
                    height = GuiLocation.Height;
                    left = key.GetKey("Left", left);
                    width = key.GetKey("Width", width);
                    top = key.GetKey("Top", top);
                    height = key.GetKey("Height", height);
                    SetProperty(new Rect(new Point(left, top), new Size(width, height)), nameof(GuiLocation));
                }
            }
        }



        public void Save(RegistryKey baseKey)
        {
            using (RegistryKey key = OpenGuiLocationRegKey(true))
            {
                key.SetKey("Left", GuiLocation.Left);
                key.SetKey("Width", GuiLocation.Width);
                key.SetKey("Top", GuiLocation.Top);
                key.SetKey("Height", GuiLocation.Height);
            }

            using (RegistryKey key = OpenMonitorRegKey(true))
            {
                if (key != null)
                {
                    SaveRotatedValue(SideNames, "%Border", "Left", GetProperty<double>(nameof(RealLeftBorder)));
                    SaveRotatedValue(SideNames, "%Border", "Top", GetProperty<double>(nameof(RealTopBorder)));
                    SaveRotatedValue(SideNames, "%Border", "Right", GetProperty<double>(nameof(RealRightBorder)));
                    SaveRotatedValue(SideNames, "%Border", "Bottom", GetProperty<double>(nameof(RealBottomBorder)));

                    SaveRotatedValue(DimNames, "Physical%", "Width", GetProperty<double>(nameof(RealPhysicalWidth)));
                    SaveRotatedValue(DimNames, "Physical%", "Height", GetProperty<double>(nameof(RealPhysicalHeight)));

                    key.SetKey("PnpName", PnpDeviceName);
                    key.SetKey("DeviceId", Monitor.DeviceId);
                }
            }

            using (RegistryKey key = OpenConfigRegKey(true))
            {
                if (key != null)
                {
                    key.SetKey( nameof(PhysicalX),GetProperty<double>(nameof(PhysicalX)));
                    key.SetKey( nameof(PhysicalY), GetProperty<double>(nameof(PhysicalY)));
                    key.SetKey( nameof(PhysicalRatioX), GetProperty<double>(nameof(PhysicalRatioX)) );
                    key.SetKey( nameof(PhysicalRatioY), GetProperty<double>(nameof(PhysicalRatioY)));

                    key.SetKey("PixelX", PixelLocation.X);
                    key.SetKey("PixelY", PixelLocation.Y);
                    key.SetKey("PixelWidth", PixelSize.Width);
                    key.SetKey("PixelHeight", PixelSize.Width);

                    key.SetKey("Primary", Primary);
                    key.SetKey("Orientation", Monitor.DisplayOrientation);
                }
            }
        }


        public AbsoluteRectangle AbsoluteWorkingArea => new AbsoluteRectangle(
            new PixelPoint(Config, this, Monitor.WorkArea.X, Monitor.WorkArea.Y),
            new PixelPoint(Config, this, Monitor.WorkArea.Right, Monitor.WorkArea.Bottom)
            );

        private static readonly List<string> SideNames = new List<string> { "Left", "Top", "Right", "Bottom" };
        private static readonly List<string> DimNames = new List<string> { "Width", "Height" };

        private double LoadRotatedValue(List<string> names, string name, string side, double def, bool fromConfig=false)
        {
            int n = names.Count;
            int pos = (names.IndexOf(side) + Monitor.DisplayOrientation) % n;
            using (RegistryKey key = fromConfig?OpenConfigRegKey():OpenMonitorRegKey())
            {
                return key.GetKey(name.Replace("%", names[pos]), def);
            }
        }


        private void SaveRotatedValue(List<string> names, string name, string side, double value, bool fromConfig = false)
        {
            int n = names.Count;
            int pos = (names.IndexOf(side) + n - Monitor.DisplayOrientation) % n;
            using (RegistryKey key = fromConfig ? OpenConfigRegKey(true) : OpenMonitorRegKey(true))
            {
                key.SetKey(name.Replace("%", names[pos]), value);
            }
        }


        public double RealLeftBorder
        {
            get { return GetProperty<double>(); }
            set { SetProperty((value < 0)?0:value); }
        }
        public double RealLeftBorder_default => LoadRotatedValue(SideNames,"%Border", "Left", 20.0);

        public double RealRightBorder
        {
            get { return GetProperty<double>(); }
            set { SetProperty((value < 0) ? 0 : value); }
        }
        public double RealRightBorder_default => LoadRotatedValue(SideNames, "%Border", "Right", 20.0);

        public double RealTopBorder
        {
            get { return GetProperty<double>(); }
            set { SetProperty((value < 0) ? 0 : value); }
        }
        public double RealTopBorder_default => LoadRotatedValue(SideNames, "%Border", "Top", 20.0);

        public double RealBottomBorder
        {
            get { return GetProperty<double>(); }
            set { SetProperty((value < 0) ? 0 : value); }
        }
        public double RealBottomBorder_default => LoadRotatedValue(SideNames, "%Border", "Bottom", 20.0);

        [DependsOn(nameof(RealLeftBorder), nameof(PhysicalRatioX))]
        public double LeftBorder => RealLeftBorder * PhysicalRatioX;

        [DependsOn(nameof(RealRightBorder), nameof(PhysicalRatioX))]
        public double RightBorder => RealRightBorder * PhysicalRatioX;

        [DependsOn(nameof(RealTopBorder), nameof(PhysicalRatioY))]
        public double TopBorder => RealTopBorder * PhysicalRatioY;

        [DependsOn(nameof(RealBottomBorder), nameof(PhysicalRatioY))]
        public double BottomBorder => RealBottomBorder * PhysicalRatioY;


        public Rect PhysicalOutsideBounds
        {
            get { return GetProperty<Rect>(); }
            private set { SetProperty(value); }
        }

        public Rect PhysicalOutsideBounds_default
        {
            get
            {
                    double x = PhysicalX - LeftBorder;
                    double y = PhysicalY - TopBorder;
                    double w = PhysicalWidth + LeftBorder + RightBorder;
                    double h = PhysicalHeight + TopBorder + BottomBorder;
                    return new Rect(new Point(x, y), new Size(w, h));
            }
        }

        [DependsOn(
            nameof(PhysicalX), nameof(PhysicalY), 
            nameof(LeftBorder), nameof(TopBorder),nameof(RightBorder) , nameof(BottomBorder),
            nameof(PhysicalWidth), nameof(PhysicalHeight))]
        public void UpdatePhysicalOutsideBounds()
        {
            PhysicalOutsideBounds = PhysicalOutsideBounds_default;
        }

        public Rect GuiLocation
        {
            get { return GetProperty<Rect>(); }
            set { SetProperty(value); }
        }

        public Rect GuiLocation_default => new Rect
                    {
                        Width = 0.5,
                        Height = (9*Monitor.WorkArea.Width/16)/Monitor.WorkArea.Height,
                        Y = 1 - (9 * Monitor.WorkArea.Width / 16) / Monitor.WorkArea.Height,
                        X = 1 - 0.5,
                    };

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

        public Point ScaledPoint(Point p)
        {
            NativeMethods.POINT up = new NativeMethods.POINT((int)p.X, (int)p.Y);
            NativeMethods.PhysicalToLogicalPoint(Monitor.HMonitor, ref up);
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

        private NativeMethods.Process_DPI_Awareness DpiAwareness
        {
            get
            {
                Process p = Process.GetCurrentProcess();

                NativeMethods.Process_DPI_Awareness aw = NativeMethods.Process_DPI_Awareness.Process_Per_Monitor_DPI_Aware;

                NativeMethods.GetProcessDpiAwareness(p.Handle, out aw);

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
                    case NativeMethods.Process_DPI_Awareness.Process_DPI_Unaware:
                        return Math.Round((RealDpiX / DpiAwareAngularDpiX) * 20) / 20;
                    case NativeMethods.Process_DPI_Awareness.Process_System_DPI_Aware:
                        return Config.PrimaryScreen.EffectiveDpiX / 96;
                    case NativeMethods.Process_DPI_Awareness.Process_Per_Monitor_DPI_Aware:
                        return Config.MaxEffectiveDpiX / 96;
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
                    case NativeMethods.Process_DPI_Awareness.Process_DPI_Unaware:
                        return Math.Round((RealDpiY / DpiAwareAngularDpiY) * 20) / 20;
                    case NativeMethods.Process_DPI_Awareness.Process_System_DPI_Aware:
                        return Config.PrimaryScreen.EffectiveDpiY / 96;
                    case NativeMethods.Process_DPI_Awareness.Process_Per_Monitor_DPI_Aware:
                        return Config.MaxEffectiveDpiY / 96;
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

        [DependsOn("PhysicalToPixelRatioY", "WpfToPixelRatioY")]
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
                IntPtr hdc = NativeMethods.CreateDC("DISPLAY", Monitor.Adapter.DeviceName, null, IntPtr.Zero);
                double dpi = NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.LOGPIXELSX);
                NativeMethods.DeleteDC(hdc);
                return dpi;
            }
        }

        public string CapabilitiesString
        {
            get
            {
                IntPtr hMonitor = Monitor.HMonitor; //HPhysical;

                uint len = 0;
                if (!NativeMethods.DDCCIGetCapabilitiesStringLength(hMonitor, ref len)) return "-1-";

                StringBuilder s = new StringBuilder((int)len + 1);

                if (!NativeMethods.DDCCIGetCapabilitiesString(hMonitor, s, len)) return "-2-";

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

