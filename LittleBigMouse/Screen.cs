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
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
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
        private void changed(String name)
        {
            if (PropertyChanged != null) PropertyChanged(this, new PropertyChangedEventArgs(name));
        }
        private void changed(Object o, String name)
        {
            if (PropertyChanged != null) PropertyChanged(o, new PropertyChangedEventArgs(name));
        }

        public event EventHandler PhysicalChanged;

        private void OnPhysicalChanged()
        {
            if (PhysicalChanged != null) PhysicalChanged(this, new EventArgs());
        }

        internal System.Windows.Forms.Screen _screen;
        private ScreenConfig _config = null;
        internal Edid _edid;

        internal Screen(ScreenConfig config, System.Windows.Forms.Screen screen)
        {
            _config = config;
            _screen = screen;
            _edid = new Edid(screen.DeviceName);

            _brightness = new MonitorLevel(this, Dxva2.GetMonitorBrightness, Dxva2.SetMonitorBrightness);
            _contrast = new MonitorLevel(this, Dxva2.GetMonitorContrast, Dxva2.SetMonitorContrast);

            _redGain = new MonitorLevel(this, Dxva2.GetMonitorRedGreenOrBlueGain, Dxva2.SetMonitorRedGreenOrBlueGain, 0);
            _greenGain = new MonitorLevel(this, Dxva2.GetMonitorRedGreenOrBlueGain, Dxva2.SetMonitorRedGreenOrBlueGain, 1);
            _blueGain = new MonitorLevel(this, Dxva2.GetMonitorRedGreenOrBlueGain, Dxva2.SetMonitorRedGreenOrBlueGain, 2);

            _redDrive = new MonitorLevel(this, Dxva2.GetMonitorRedGreenOrBlueDrive, Dxva2.SetMonitorRedGreenOrBlueDrive, 0);
            _greenDrive = new MonitorLevel(this, Dxva2.GetMonitorRedGreenOrBlueDrive, Dxva2.SetMonitorRedGreenOrBlueDrive, 1);
            _blueDrive = new MonitorLevel(this, Dxva2.GetMonitorRedGreenOrBlueDrive, Dxva2.SetMonitorRedGreenOrBlueDrive, 2);
        }
        public IntPtr HMonitor
        {
            get
            {
                IntPtr h = IntPtr.Zero;

                User32.EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero,
                    delegate (IntPtr hMonitor, IntPtr hdcMonitor, ref RECT lprcMonitor, IntPtr dwData)
                    {
                        MONITORINFOEX mi = new MONITORINFOEX();
                        mi.Size = Marshal.SizeOf(mi);
                        bool success = User32.GetMonitorInfo(hMonitor, ref mi);
                        if (success && mi.DeviceName == DeviceName)
                        {
                            h = hMonitor;
                            return false;
                        }
                        return true;
                    }, IntPtr.Zero);
                return h;
            }
        }

        public String ProductCode
        {
            get { return _edid.ProductCode; }
        }

        public String Model
        {
            get { return _edid.Block((char)0xFC); }
        }
        public String ManufacturerCode
        {
            get { return _edid.ManufacturerCode; }
        }
        public String PNPCode
        {
            get { return ManufacturerCode + ProductCode; }
        }

        public String SerialNo
        {
            get { return _edid.Block((char)0xFF); }
        }

        public String Serial
        {
            get { return _edid.Serial; }
        }

        public String ID
        {
            get
            {
                return _edid.ManufacturerCode + ProductCode.ToString() + "_" + Serial.ToString() + "_" + Bounds.Width + "x" + Bounds.Height;
            }
        }

        public void Save(RegistryKey baseKey)
        {
            using (RegistryKey key = baseKey.CreateSubKey(ID))
            {
                key.SetValue("X", PhysicalLocation.X.ToString(), RegistryValueKind.String);
                key.SetValue("Y", PhysicalLocation.Y.ToString(), RegistryValueKind.String);

                if (double.IsNaN(_pitchX)) { key.DeleteValue("PitchX", false); }
                else { key.SetValue("PitchX", PitchX.ToString(), RegistryValueKind.String); }

                if (double.IsNaN(_pitchY)) { key.DeleteValue("PitchY", false); }
                else { key.SetValue("PitchY", PitchY.ToString(), RegistryValueKind.String); }

                key.Close();
            }

        }
        public void Load(RegistryKey configkey)
        {
            using (RegistryKey key = configkey.OpenSubKey(ID))
            {
                if (key != null)
                {
                    PhysicalX = double.Parse(key.GetValue("X", RegistryValueKind.String).ToString());
                    PhysicalY = double.Parse(key.GetValue("Y", RegistryValueKind.String).ToString());

                    String pitchX = key.GetValue("PitchX", "NaN").ToString();
                    if (pitchX != "NaN") PitchX = double.Parse(pitchX);

                    String pitchY = key.GetValue("PitchY", "NaN").ToString();
                    if (pitchY != "NaN") PitchY = double.Parse(pitchY);

                    key.Close();
                }
                else
                {
                    // TODO : try to define position from Windows one
                    PhysicalX = Config.PhysicalOverallBounds.Right;
                    PhysicalY = 0;
                }
            }
        }

        public Rect Bounds
        {
            get { return getRect(_screen.Bounds); }
        }

        public Point Location
        {
            get { return Bounds.TopLeft; }
            set
            {
                if (Primary)
                {
                    foreach (Screen s in Config.AllScreens)
                    {
                        if (!s.Primary)
                        {
                            s.Location = new Point(s.Location.X - value.X, s.Location.Y - value.Y);
                        }
                    }
                }
                else
                {
                    DEVMODE devmode = new DEVMODE();
                    devmode.Size = (short)Marshal.SizeOf(devmode);

                    devmode.Position.x = (int)value.X;
                    devmode.Position.y = (int)value.Y;
                    devmode.Fields = DM.Position;

                    DISP_CHANGE result = User32.ChangeDisplaySettingsEx(
                        DeviceName,
                        ref devmode,
                        IntPtr.Zero,
                        0,
                        IntPtr.Zero
                        );
                }
            }
        }

        public void AlignScreens(Point p)
        {
            Point phy = PixelToPhysical(p);
            foreach (Screen s in Config.AllScreens)
            {
                if (s != this && s.PhysicalBounds.Top < PhysicalBounds.Bottom && s.PhysicalBounds.Bottom > PhysicalBounds.Top)
                {
                    Point dest = s.PhysicalToPixel(phy);

                    double offset = dest.Y - p.Y;

                    s.Location = new Point(s.Location.X, s.Location.Y + offset);
                }
            }
        }

        public Rect InsideBounds
        {
            get { return new Rect(Bounds.TopLeft, new Point(Bounds.BottomRight.X - 1.0, Bounds.BottomRight.Y - 1.0)); }
        }

        private double _physicalX = 0;
        private double _physicalY = 0;
        public Point PhysicalLocation
        {
            get
            {
                return new Point(_physicalX, _physicalY);
            }
            set
            {
                PhysicalX = value.X;
                PhysicalY = value.Y;
            }
        }

        public double PhysicalX
        {
            get { return _physicalX; }
            set
            {
                if (Primary)
                {
                    foreach (Screen s in Config.AllScreens)
                    {
                        if (!s.Primary)
                        {
                            s.PhysicalX -= value;
                        }
                    }
                    _physicalX = 0;
                }
                else
                {
                    _physicalX = value;
                }
                OnPhysicalChanged();
                changed("PhysicalX");
                changed("PhysicalLocation");
                changed("PhysicalBounds");
            }
        }
        public double PhysicalY
        {
            get { return _physicalY; }
            set
            {
                if (Primary)
                {
                    foreach (Screen s in Config.AllScreens)
                    {
                        if (!s.Primary)
                        {
                            s.PhysicalY -= value;
                        }
                    }
                    _physicalY = 0;
                }
                else
                {
                    _physicalY = value;
                }
                OnPhysicalChanged();
                changed("PhysicalY");
                changed("PhysicalLocation");
                changed("PhysicalBounds");
            }
        }

        public Rect PhysicalBounds
        {
            get
            {
                return new Rect(PhysicalLocation, PixelToPhysical(Bounds.BottomRight));
            }
        }

        public Rect WpfBounds
        {
            get
            {
                return PixelToWpf(Bounds);
            }
        }

        public Rect WorkingArea
        {
            get { return getRect(_screen.WorkingArea); }
        }

        public Rect WpfWorkingArea
        {
            get { return PixelToWpf(WorkingArea); }
        }

        private Rect getRect(System.Drawing.Rectangle value)
        {
            // should x, y, width, height be device-independent-pixels ??
            return new Rect
            {
                X = value.X,
                Y = value.Y,
                Width = value.Width,
                Height = value.Height
            };
        }

        public bool Primary
        {
            get { return _screen.Primary; }
        }

        public string DeviceName
        {
            get { return _screen.DeviceName; }
        }

        public int DeviceNo
        {
            get { return int.Parse(_screen.DeviceName.Substring(11)); }
        }

        double _pitchX = double.NaN;
        public double PitchX
        {
            set
            {
                if (value != PitchX)
                {
                    _pitchX = value;
                    changed("PitchX");
                    changed("DpiX");
                    changed("DpiAvg");
                    changed("PhysicalWidth");
                    changed("PhysicalBounds");
                }
            }
            get
            {
                if (double.IsNaN(_pitchX))
                    return DeviceCapsPhysicalSize.Width / Bounds.Width;
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
                if (value != PitchY)
                {
                    _pitchY = value;
                    changed("PitchY");
                    changed("DpiY");
                    changed("DpiAvg");
                    changed("PhysicalHeight");
                    changed("PhysicalBounds");
                }
            }
            get
            {
                if (double.IsNaN(_pitchY))
                    return DeviceCapsPhysicalSize.Height / Bounds.Height;
                else return _pitchY;
                //                if (_edid.IsValid) return _edid.PhysicalSize.Height / Bounds.Height;
                //                else return 25.4 / 96.0;
            }
        }

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

        public double PhysicalWidth
        {
            set { PitchX = value / Bounds.Width; }
            get { return PitchX * Bounds.Width; }
        }
        public double PhysicalHeight
        {
            set { PitchY = value / Bounds.Height; }
            get { return PitchY * Bounds.Height; }
        }

        private double GraphicsDpiX
        {
            get
            {
                IntPtr hdc = Gdi32.CreateDC(null, _screen.DeviceName, null, IntPtr.Zero);
                System.Drawing.Graphics gfx = System.Drawing.Graphics.FromHdc(hdc);
                double dpix = gfx.DpiX;
                gfx.Dispose();
                Gdi32.DeleteDC(hdc);
                return dpix;
            }
        }
        private double GraphicsDpiY
        {
            get
            {
                IntPtr hdc = Gdi32.CreateDC(null, _screen.DeviceName, null, IntPtr.Zero);
                System.Drawing.Graphics gfx = System.Drawing.Graphics.FromHdc(hdc);
                double dpiy = gfx.DpiY;
                gfx.Dispose();
                Gdi32.DeleteDC(hdc);
                return dpiy;
            }
        }

        public double PixelToWpfRatioX
        {
            get
            {
                return 96.0 / GraphicsDpiX;
            }
        }
        public double PixelToWpfRatioY
        {
            get
            {
                return 96.0 / GraphicsDpiY;
            }
        }

        private double LogPixelSx
        {
            get
            {
                IntPtr hdc = Gdi32.CreateDC("DISPLAY", _screen.DeviceName, null, IntPtr.Zero);
                double dpi = Gdi32.GetDeviceCaps(hdc, DeviceCap.LOGPIXELSX);
                Gdi32.DeleteDC(hdc);
                return dpi;
            }
        }
        private Size DeviceCapsPhysicalSize
        {
            get
            {
                IntPtr hdc = Gdi32.CreateDC("DISPLAY", _screen.DeviceName, null, IntPtr.Zero);
                double w = Gdi32.GetDeviceCaps(hdc, DeviceCap.HORZSIZE);
                double h = Gdi32.GetDeviceCaps(hdc, DeviceCap.VERTSIZE);
                Gdi32.DeleteDC(hdc);
                return new Size(w, h);
            }
        }

        internal ScreenConfig Config
        {
            get
            {
                return _config;
            }
        }

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



        public Point PixelToPhysical(Point p)
        {
            double x = (p.X - Bounds.X) * PitchX + PhysicalLocation.X;
            double y = (p.Y - Bounds.Y) * PitchY + PhysicalLocation.Y;
            return new Point(x, y);
        }
        public Size PixelToPhysical(Size s)
        {
            double x = s.Width * PitchX + PhysicalLocation.X;
            double y = s.Height * PitchY + PhysicalLocation.Y;
            return new Size(x, y);
        }
        public Point PhysicalToPixel(Point p)
        {
            double x = Math.Floor(((p.X - PhysicalLocation.X) / PitchX) + 0.5) + Bounds.X;
            double y = Math.Floor(((p.Y - PhysicalLocation.Y) / PitchY) + 0.5) + Bounds.Y;
            return new Point(x, y);
        }

        public Point PixelToWpf(Point p)
        {
            double x = p.X * PixelToWpfRatioX;
            double y = p.Y * PixelToWpfRatioY;
            return new Point(x, y);
        }

        public Rect PixelToWpf(Rect r)
        {
            return new Rect(PixelToWpf(r.TopLeft), PixelToWpf(r.BottomRight));
        }

        public Point WpfToPixel(Point p)
        {
            double x = Math.Floor((p.X / PixelToWpfRatioX) + 0.5);
            double y = Math.Floor((p.Y / PixelToWpfRatioY) + 0.5);
            return new Point(x, y);
        }
        public Point PhysicalToWpf(Point p)
        {
            return PixelToWpf(PhysicalToPixel(p));
        }
        public Point WpfToPhysical(Point p)
        {
            return PixelToPhysical(WpfToPixel(p));
        }

        public IntPtr HPhysical
        {
            get
            {
                IntPtr hMonitor = HMonitor;
                MONITORINFOEX monitorInfoEx = new MONITORINFOEX();
                monitorInfoEx.Size = (int)Marshal.SizeOf(monitorInfoEx);

                if (User32.GetMonitorInfo(hMonitor, ref monitorInfoEx))
                {
                    uint pdwNumberOfPhysicalMonitors = 0;
                    Dxva2.GetNumberOfPhysicalMonitorsFromHMONITOR(hMonitor, ref pdwNumberOfPhysicalMonitors);

                    if (pdwNumberOfPhysicalMonitors >= 0)
                    {
                        // Récupère un handle physique du moniteur
                        PHYSICAL_MONITOR[] pPhysicalMonitorArray = new PHYSICAL_MONITOR[pdwNumberOfPhysicalMonitors];
                        Dxva2.GetPhysicalMonitorsFromHMONITOR(hMonitor, pdwNumberOfPhysicalMonitors, pPhysicalMonitorArray);
                        //Nom = pPhysicalMonitorArray[0].szPhysicalMonitorDescription;
                        return pPhysicalMonitorArray[0].hPhysicalMonitor;
                    }
                }
                return IntPtr.Zero;

            }

        }


        private MonitorLevel _brightness;
        private MonitorLevel _contrast;

        private MonitorLevel _redGain;
        private MonitorLevel _greenGain;
        private MonitorLevel _blueGain;

        private MonitorLevel _redDrive;
        private MonitorLevel _greenDrive;
        private MonitorLevel _blueDrive;

        public MonitorLevel Brightness { get { return _brightness; } }
        public MonitorLevel Contrast { get { return _contrast; } }

        public MonitorLevel RedGain { get { return _redGain; } }
        public MonitorLevel GreenGain { get { return _greenGain; } }
        public MonitorLevel BlueGain { get { return _blueGain; } }

        public MonitorLevel RedDrive { get { return _redDrive; } }
        public MonitorLevel GreenDrive { get { return _greenDrive; } }
        public MonitorLevel BlueDrive { get { return _blueDrive; } }
    }

    public delegate bool MonitorGetter(IntPtr h, ref uint min, ref uint value, ref uint max);
    public delegate bool MonitorComponentGetter(IntPtr h, uint component, ref uint min, ref uint value, ref uint max);
    public delegate bool MonitorSetter(IntPtr h, uint value);
    public delegate bool MonitorComponentSetter(IntPtr h, uint component, uint value);

    public class MonitorLevel : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private void changed(String name)
        {
            if (PropertyChanged != null) PropertyChanged(this, new PropertyChangedEventArgs(name));
        }

        Screen _screen;

        uint _value = 0;
        uint _min = 0;
        uint _max = 0;
        uint _pendding = uint.MaxValue;

        uint _component = 0;

        MonitorSetter _setter = null;
        MonitorGetter _getter = null;
        MonitorComponentSetter _componentSetter = null;
        MonitorComponentGetter _componentGetter = null;

        public MonitorLevel(Screen screen, MonitorGetter getter, MonitorSetter setter)
        {
            _screen = screen;
            _setter = setter;
            _getter = getter;

           getValueThread();
        }
        public MonitorLevel(Screen screen, MonitorComponentGetter getter, MonitorComponentSetter setter, uint component)
        {
            _screen = screen;
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

            if (_screen != null)
            {
                if (_getter != null)
                    _getter(_screen.HPhysical, ref min, ref value, ref max);
                else if (_componentGetter != null)
                    _componentGetter(_screen.HPhysical, _component, ref min, ref value, ref max);

            }

            if (min != _min) { _min = min; changed("Min"); }
            if (max != _max) { _max = max; changed("Max"); }
            if (value != _value) { _value = value; changed("Value"); }
        }
        private void getValueThread()
        {
            Thread thread = new Thread(
            new ThreadStart(
              delegate ()
              {
                  getValue();
                  if (_pendding!=uint.MaxValue && _value!=_pendding)
                  {
                      setValueThread();
                  }
              }
              ));

            thread.Start();
        }

        Thread _threadSetter;
        private void setValueThread()
        {
            if (_threadSetter==null || !_threadSetter.IsAlive)
            {
                _threadSetter = new Thread(
                new ThreadStart(
                  delegate ()
                  {
                      if (_pendding!=uint.MaxValue)
                      {
                          if (_setter != null)
                              _setter(_screen.HPhysical, _pendding);
                          else if (_componentSetter != null)
                              _componentSetter(_screen.HPhysical, _component, _pendding);

                          _pendding = uint.MaxValue;
                      }

                      getValueThread();
                  }
                  ));

                _threadSetter.Start();
            }

        }
        public uint Value
        {
            get { return _value; }
            set
            {
                if (_pendding!=value)
                {
                    _pendding = value;
                    setValueThread();
                }
            }
        }
        public uint Min
        {
            get { return _min; }
        }
        public uint Max
        {
            get { return _max; }
        }
    }

}

