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
using System.Globalization;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows;
using WinAPI_Dxva2;
using WinAPI_Gdi32;
using WinAPI_User32;

[assembly: InternalsVisibleTo("ScreenConfig")]
namespace LittleBigMouse
{
    public class Screen : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private void Changed(string name)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
        }

        private void Changed(object o, string name)
        {
            PropertyChanged?.Invoke(o, new PropertyChangedEventArgs(name));
        }

        public event EventHandler PhysicalChanged;

        private void OnPhysicalChanged()
        {
            PhysicalChanged?.Invoke(this, new EventArgs());
        }

        internal System.Windows.Forms.Screen FormScreen;
        internal Edid Edid;

        internal Screen(ScreenConfig config, System.Windows.Forms.Screen screen)
        {
            Config = config;
            FormScreen = screen;
            Edid = new Edid(this);

            // TODO : try to define position from Windows one
            PhysicalLocation = new PhysicalPoint(
                this,
                PhysicalOveralBoundsWithoutThis.Right,
                0);


            Brightness = new MonitorLevel(GetBrightness, SetBrightness);
            Contrast = new MonitorLevel(GetContrast, SetContrast);

            Gain = new MonitorRGBLevel(GetGain, SetGain);
            Drive = new MonitorRGBLevel(GetDrive, SetDrive);

            Probe = new MonitorRGBLevel(GetProbe, SetProbe);
        }

        ~Screen()
        {
            if (_pPhysicalMonitorArray != null && _pPhysicalMonitorArray.Length > 0)
                Dxva2.DestroyPhysicalMonitors((uint)_pPhysicalMonitorArray.Length,ref _pPhysicalMonitorArray);
        }


        private IntPtr _hmonitor = IntPtr.Zero;
        public IntPtr HMonitor
        {
            get
            {
                if (_hmonitor != IntPtr.Zero) return _hmonitor;

                User32.EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero,
                    delegate (IntPtr hMonitor, IntPtr hdcMonitor, ref RECT lprcMonitor, IntPtr dwData)
                    {
                        MONITORINFOEX mi = new MONITORINFOEX();
                        mi.Size = Marshal.SizeOf(mi);
                        bool success = User32.GetMonitorInfo(hMonitor, ref mi);
                        if (success && mi.DeviceName == DeviceName)
                        {
                            _hmonitor = hMonitor;
                            return false;
                        }
                        return true;
                    }, IntPtr.Zero);
                return _hmonitor;
            }
        }

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

        public string ProductCode => Edid.ProductCode;
        public string Model => Edid.Block((char)0xFC);

        public string ManufacturerCode => Edid.ManufacturerCode;

        public string PnpCode => ManufacturerCode + ProductCode;

        public string SerialNo => Edid.Block((char)0xFF);

        public string Serial => Edid.Serial;

        public string Id => Edid.ManufacturerCode + ProductCode + "_" + Serial + "_" + PixelSize.Width + "x" + PixelSize.Height;

        public void Save(RegistryKey baseKey)
        {
            using (RegistryKey key = baseKey.CreateSubKey(Id))
            {
                if (key == null) return;

                key.SetValue("X", PhysicalLocation.X.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String);
                key.SetValue("Y", PhysicalLocation.Y.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String);

                if (double.IsNaN(_pitchX)) { key.DeleteValue("PitchX", false); }
                else { key.SetValue("PitchX", PitchX.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String); }

                if (double.IsNaN(_pitchY)) { key.DeleteValue("PitchY", false); }
                else { key.SetValue("PitchY", PitchY.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String); }
            }

        }

        public Rect PhysicalOveralBoundsWithoutThis
        {
            get
            {
                Rect r = new Rect();
                bool first = true;
                foreach (Screen s in Config.AllScreens.Where(s => s!=this))
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

        public void Load(RegistryKey configkey)
        {
            using (RegistryKey key = configkey.OpenSubKey(Id))
            {
                if (key == null) return;

                string sX = key.GetValue("X", RegistryValueKind.String).ToString();
                double x = double.Parse(sX, CultureInfo.InvariantCulture);

                string sY = key.GetValue("Y", RegistryValueKind.String).ToString();
                double y = double.Parse(sY, CultureInfo.InvariantCulture);

                PhysicalLocation = new PhysicalPoint(this,x,y);

                string pitchX = key.GetValue("PitchX", "NaN").ToString();
                if (pitchX != "NaN") PitchX = double.Parse(pitchX, CultureInfo.InvariantCulture);

                string pitchY = key.GetValue("PitchY", "NaN").ToString();
                if (pitchY != "NaN") PitchY = double.Parse(pitchY, CultureInfo.InvariantCulture);
            }
        }


        private PixelPoint _pixelLocation;
        private Size _pixelSize;

        private void GetPixelBounds(bool force = false)
        {
            if (!(force || _pixelLocation == null)) return;

            var dev = new DEVMODE();

            User32.EnumDisplaySettings(DeviceName, -1, ref dev);

            _pixelLocation = new PixelPoint(this, dev.Position.x, dev.Position.y);
            _pixelSize = new Size(dev.PelsWidth, dev.PelsHeight);
        }

        public AbsolutePoint Location => PixelLocation;

        public Size PixelSize
        {
            get
            {
                GetPixelBounds();
                return _pixelSize;
                ;
            }
        } 
        public AbsolutePoint BottomRight => new PixelPoint(this, PixelLocation.X + PixelSize.Width, PixelLocation.Y + PixelSize.Height);

        public AbsoluteRectangle Bounds => new AbsoluteRectangle(Location, BottomRight);
 


        private PhysicalPoint _physicalLocation = null;
 


    public PixelPoint PixelLocation
        {
            get
            {
                GetPixelBounds();
                return _pixelLocation;
            }
            set
            {
                if (Primary)
                {
                    foreach (var s in Config.AllScreens.Where(s => !s.Primary))
                    {
                        s.PixelLocation = new PixelPoint(this,s.PixelLocation.X - value.X, s.PixelLocation.Y - value.Y);
                    }
                }
                else
                {
                    var devmode = new DEVMODE();
                    devmode.Size = (short)Marshal.SizeOf(devmode);

                    devmode.Position.x = (int)value.X;
                    devmode.Position.y = (int)value.Y;
                    devmode.Fields = DM.Position;

                    var result = User32.ChangeDisplaySettingsEx(
                        DeviceName,
                        ref devmode,
                        IntPtr.Zero,
                        0,
                        IntPtr.Zero
                        );

                    if (result == DISP_CHANGE.Successful)
                    {
                        GetPixelBounds(true);
                    }
                }
            }
        }
/*
        public void AlignScreens(Point p)
        {
            var phy = PixelToPhysical(p);
            foreach (var s in Config.AllScreens)
            {
                if (s == this || !(s.PhysicalBounds.Top < PhysicalBounds.Bottom) ||
                    !(s.PhysicalBounds.Bottom > PhysicalBounds.Top)) continue;

                var dest = s.PhysicalToPixel(phy);
                var offset = dest.Y - p.Y;
                s.PixelLocation = new PixelPoint(this, s.PixelLocation.X, s.PixelLocation.Y + offset);
            }
        }
        */


        public double PhysicalX
        {
            get { return PhysicalLocation.X; }
            set
            {
                PhysicalLocation = new PhysicalPoint(PhysicalLocation.Screen, value, PhysicalLocation.Y);
            }
        }
        public double PhysicalY
        {
            get { return PhysicalLocation.Y; }
            set
            {
                PhysicalLocation = new PhysicalPoint(PhysicalLocation.Screen, PhysicalLocation.X, value);
            }
        }

        public double PixelWidth => Bounds.BottomRight.Pixel.X - Bounds.TopLeft.Pixel.X;
        public double PixelHeight => Bounds.BottomRight.Pixel.Y - Bounds.TopLeft.Pixel.Y;

        public PhysicalPoint PhysicalLocation
        {
            get
            {
                GetPixelBounds();
                return _physicalLocation??new PhysicalPoint(this,0,0);
            }
            set
            {
                if (value == _physicalLocation) return;

                if (Primary)
                {
                    foreach (Screen s in Config.AllScreens)
                    {
                        if (s.Primary) continue;
                        double x = s.PhysicalLocation.X - value.X;
                        double y = s.PhysicalLocation.Y - value.Y;
                        s.PhysicalLocation = new PhysicalPoint(s,x,y);
                    }
                }
                else
                    _physicalLocation = value;

                Changed("PhysicalLocation");
                Changed("PhysicalBounds");
            }
        }

        public Rect PhysicalBounds => new Rect(
            PhysicalLocation.Point,
            Bounds.BottomRight.Physical.Point
            );

//        public Rect WpfBounds => new Rect(PixelLocation.Wpf.Point, Bounds.BottomRight.Wpf.Point);

        public Rect WorkingArea => GetRect(FormScreen.WorkingArea);


        private static Rect GetRect(System.Drawing.Rectangle value)
        {
            return new Rect
            {
                X = value.X,
                Y = value.Y,
                Width = value.Width,
                Height = value.Height
            };
        }

        public bool Primary => FormScreen.Primary;

        public string DeviceName => FormScreen.DeviceName;

        public int DeviceNo => int.Parse(FormScreen.DeviceName.Substring(11));

        double _pitchX = double.NaN;
        public double PitchX
        {
            set
            {
                if (value == PitchX) return;
                _pitchX = value;
                Changed("PitchX");
                Changed("DpiX");
                Changed("DpiAvg");
                Changed("PhysicalWidth");
                Changed("PhysicalBounds");
            }
            get
            {
                if (double.IsNaN(_pitchX))
                    return DeviceCapsPhysicalSize.Width / PixelSize.Width;
                else return _pitchX;
                //                if (_edid.IsValid) return _edid.PhysicalSize.Width / Bounds.Width;
                //                else return 25.4 / 96.0;
            }
        }

        double _pitchY = double.NaN;
        public double PitchY
        {
            set
            {
                if (value == PitchY) return;
                _pitchY = value;
                Changed("PitchY");
                Changed("DpiY");
                Changed("DpiAvg");
                Changed("PhysicalHeight");
                Changed("PhysicalBounds");
            }
            get
            {
                if (double.IsNaN(_pitchY))
                    return DeviceCapsPhysicalSize.Height / PixelSize.Height;
                else return _pitchY;
                //                if (_edid.IsValid) return _edid.PhysicalSize.Height / Bounds.Height;
                //                else return 25.4 / 96.0;
            }
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
                return (double) factor/100;
            }
        }

        public Point ScaledPoint(Point p)
        {
            User32.POINT up = new User32.POINT((int)p.X,(int)p.Y);
            User32.PhysicalToLogicalPoint(HMonitor,ref up);
            return new Point(up.X,up.Y);
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
        public double AngularDpiX
        {
            get
            {
                uint dpiX;
                uint dpiY;
                GetDpiForMonitor(HMonitor, DpiType.Angular, out dpiX, out dpiY);
                return dpiX;
            }
        }
        public double AngularDpiY
        {
            get
            {
                uint dpiX;
                uint dpiY;
                GetDpiForMonitor(HMonitor, DpiType.Angular, out dpiX, out dpiY);
                return dpiY;
            }
        }

        public double RatioX => Math.Round((DpiX / AngularDpiX) * 20) / 20;
        public double RatioY => Math.Round((DpiY / AngularDpiY) * 20) / 20;

        public double DpiX
        {
            set { PitchX = 25.4 / value; }
            get { return 25.4 / PitchX; }
        }

        public double DpiY
        {
            set { PitchY = 25.4 / value; }
            get { return 25.4 / PitchY; }
        }

        public double DpiAvg
        {
            get
            {
                double pitch = Math.Sqrt((PitchX * PitchX) + (PitchY * PitchY)) / Math.Sqrt(2);
                return 25.4 / pitch;
            }
        }

        [Obsolete]
        public double PhysicalWidth
        {
            set { PitchX = value / PixelSize.Width; }
            get { return PitchX * PixelSize.Width; }
        }
        [Obsolete]
        public double PhysicalHeight
        {
            set { PitchY = value / PixelSize.Height; }
            get { return PitchY * PixelSize.Height; }
        }

        private double _graphicsDpiX = 0;
        private double GraphicsDpiX
        {
            get
            {
                if (_graphicsDpiX == 0)
                {
                    IntPtr hdc = Gdi32.CreateDC(null, FormScreen.DeviceName, null, IntPtr.Zero);
                    System.Drawing.Graphics gfx = System.Drawing.Graphics.FromHdc(hdc);
                    _graphicsDpiX = gfx.DpiX;
                    gfx.Dispose();
                    Gdi32.DeleteDC(hdc);

                }
                return _graphicsDpiX;
            }
        }

        private double _graphicsDpiY = 0;
        private double GraphicsDpiY
        {
            get
            {
                if (_graphicsDpiY == 0)
                {
                    IntPtr hdc = Gdi32.CreateDC(null, FormScreen.DeviceName, null, IntPtr.Zero);
                    System.Drawing.Graphics gfx = System.Drawing.Graphics.FromHdc(hdc);
                    _graphicsDpiY = gfx.DpiY;
                    gfx.Dispose();
                    Gdi32.DeleteDC(hdc);
                }
                return _graphicsDpiY;
            }
        }


        private double LogPixelSx
        {
            get
            {
                IntPtr hdc = Gdi32.CreateDC("DISPLAY", FormScreen.DeviceName, null, IntPtr.Zero);
                double dpi = Gdi32.GetDeviceCaps(hdc, DeviceCap.LOGPIXELSX);
                Gdi32.DeleteDC(hdc);
                return dpi;
            }
        }
        private Size DeviceCapsPhysicalSize
        {
            get
            {
                IntPtr hdc = Gdi32.CreateDC("DISPLAY", FormScreen.DeviceName, null, IntPtr.Zero);
                double w = Gdi32.GetDeviceCaps(hdc, DeviceCap.HORZSIZE);
                double h = Gdi32.GetDeviceCaps(hdc, DeviceCap.VERTSIZE);
                Gdi32.DeleteDC(hdc);
                return new Size(w, h);
            }
        }

        internal ScreenConfig Config { get; } = null;

        public Rect ToUI(Size s)
        {
            Rect all = Config.PhysicalOverallBounds;

            double ratio = Math.Min(
                s.Width / all.Width,
                s.Height / all.Height
                );

            return new Rect(
                (PhysicalBounds.Left - all.Left) * ratio,
                (PhysicalBounds.Top - all.Top) * ratio,
                PhysicalBounds.Width * ratio,
                PhysicalBounds.Height * ratio
                );
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

        public MonitorLevel Brightness { get; }
        public MonitorLevel Contrast { get; }
        public MonitorRGBLevel Gain { get; }
        public MonitorRGBLevel Drive { get; }
        public MonitorRGBLevel Probe { get; }

        public bool GetBrightness(ref uint min, ref uint value, ref uint max)
        {
            return Dxva2.GetMonitorBrightness(HPhysical, ref min, ref value, ref max);
        }
        public bool SetBrightness(uint value)
        {
            return Dxva2.SetMonitorBrightness(HPhysical, value);
        }
        public bool GetContrast(ref uint min, ref uint value, ref uint max)
        {
            return Dxva2.GetMonitorContrast(HPhysical, ref min, ref value, ref max);
        }
        public bool SetContrast(uint value)
        {
            return Dxva2.SetMonitorContrast(HPhysical, value);
        }
        public bool GetGain(uint component, ref uint min, ref uint value, ref uint max)
        {
            return Dxva2.GetMonitorRedGreenOrBlueGain(HPhysical, component, ref min, ref value, ref max);
        }
        public bool SetGain(uint component, uint value)
        {
            return Dxva2.SetMonitorRedGreenOrBlueGain(HPhysical, component, value);
        }
        public bool GetDrive(uint component, ref uint min, ref uint value, ref uint max)
        {
            return Dxva2.GetMonitorRedGreenOrBlueDrive(HPhysical, component, ref min, ref value, ref max);
        }
        public bool SetDrive(uint component, uint value)
        {
            return Dxva2.SetMonitorRedGreenOrBlueDrive(HPhysical, component, value);
        }

        private ProbedColor _probedColor;
        public ProbedColor ProbedColor
        {
            get { return _probedColor; }
            set { _probedColor = value; Changed("ProbedColor"); }
        }
        public bool GetProbe(uint component, ref uint min, ref uint value, ref uint max)
        {
            return Dxva2.GetMonitorRedGreenOrBlueDrive(HPhysical, component, ref min, ref value, ref max);
        }
        public bool SetProbe(uint component, uint value)
        {
            return Dxva2.SetMonitorRedGreenOrBlueDrive(HPhysical, component, value);
        }
    }

    public delegate bool MonitorGetter(ref uint min, ref uint value, ref uint max);
    public delegate bool MonitorComponentGetter(uint component, ref uint min, ref uint value, ref uint max);
    public delegate bool MonitorSetter(uint value);
    public delegate bool MonitorComponentSetter(uint component, uint value);

    public class MonitorRGBLevel : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private void changed(String name)
        {
            if (PropertyChanged != null) PropertyChanged(this, new PropertyChangedEventArgs(name));
        }

        private MonitorLevel[] _values = new MonitorLevel[3];

        public MonitorRGBLevel(MonitorComponentGetter getter, MonitorComponentSetter setter)
        {
            for (uint i = 0; i < 3; i++)
                _values[i] = new MonitorLevel(getter, setter, i);

        }
        public MonitorLevel Channel(uint channel) { return _values[channel]; }
        public MonitorLevel Red => Channel(0);
        public MonitorLevel Green => Channel(1);
        public MonitorLevel Blue => Channel(2);
    }
    public class MonitorLevel : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private void changed(String name)
        {
            if (PropertyChanged != null) PropertyChanged(this, new PropertyChangedEventArgs(name));
        }

        //Screen _screen;

        uint _value = 0;
        uint _min = 0;
        uint _max = 0;
        uint _pendding = uint.MaxValue;

        uint _component = 0;

        MonitorSetter _setter = null;
        MonitorGetter _getter = null;
        MonitorComponentSetter _componentSetter = null;
        MonitorComponentGetter _componentGetter = null;


        public MonitorLevel(MonitorGetter getter, MonitorSetter setter)
        {
            //_screen = screen;
            _setter = setter;
            _getter = getter;

            getValueThread();
        }
        public MonitorLevel(MonitorComponentGetter getter, MonitorComponentSetter setter, uint component)
        {
            //_screen = screen;
            _component = component;
            _componentSetter = setter;
            _componentGetter = getter;

            getValueThread();
        }

        private void getValue()
        {
            uint min = 0;
            uint max = 0;
            uint value = 0;

            //            if (_screen != null)
            {
                if (_getter != null)
                    _getter(ref min, ref value, ref max);
                else if (_componentGetter != null)
                    _componentGetter(_component, ref min, ref value, ref max);
            }

            if (min != _min) { _min = min; changed("Min"); }
            if (max != _max) { _max = max; changed("Max"); }
            if (value != _value) { _value = value; changed("Value"); changed("ValueAsync"); }
        }
        private void getValueThread()
        {
            Thread thread = new Thread(
            new ThreadStart(
              delegate () { getValue(); }
              ));

            thread.Start();
        }

        Thread _threadSetter;
        private void setValueThread()
        {
            if (_threadSetter == null || !_threadSetter.IsAlive)
            {
                _threadSetter = new Thread(
                new ThreadStart(
                  delegate ()
                  {
                      uint pendding = _pendding;

                      if (pendding != uint.MaxValue)
                      {
                          if (_setter != null)
                              _setter(pendding);
                          else if (_componentSetter != null)
                              _componentSetter(_component, pendding);

                          getValue();
                      }

                      if (_pendding == pendding)
                          _pendding = uint.MaxValue;
                      else
                          setValueThread();
                  }
                  ));

                _threadSetter.Start();
            }
        }
        public uint ValueAsync
        {
            get { return _value; }
            set
            {
                if (_pendding != value)
                {
                    _pendding = value;
                    setValueThread();
                }
            }
        }
        public uint Value
        {
            get
            {
                while (_pendding < uint.MaxValue) ;
                return _value;
            }
            set
            {
                if (_pendding != value)
                {
                    _pendding = value;
                    setValueThread();
                    while (_pendding < uint.MaxValue) ;
                }
            }
        }
        public uint Min => _min;
        public uint Max => _max;
    }

}

