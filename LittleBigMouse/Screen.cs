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
using System.Windows;
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
        private void changed(Object o,String name)
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

                if (double.IsNaN(_pitchY)) { key.DeleteValue("PitchY",false); }
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
                    if (pitchX!="NaN") PitchX = double.Parse(pitchX);

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
            foreach(Screen s in Config.AllScreens)
            {
                if (s!=this && s.PhysicalBounds.Top<PhysicalBounds.Bottom && s.PhysicalBounds.Bottom> PhysicalBounds.Top)
                {
                    Point dest = s.PhysicalToPixel(phy);

                    double offset = dest.Y - p.Y;

                    s.Location = new Point(s.Location.X, s.Location.Y + offset);
                }
            }
        }

        public Rect InsideBounds
        {
            get { return new Rect(Bounds.TopLeft,new Point(Bounds.BottomRight.X-1.0,Bounds.BottomRight.Y-1.0)); }
        }

        private double _physicalX = 0;
        private double _physicalY = 0;
        public Point PhysicalLocation
        {
            get {
                return new Point(_physicalX,_physicalY);
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
            set {
                if (value!=PitchX)
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
            set {
                if (value!=PitchY)
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
            get { return 25.4/PitchX; }
        }

        public double DpiY
        {
            set { PitchY = 25.4 / value; }
            get { return 25.4 / PitchY; }
        }

        public double DpiAvg
        {
            get {
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

        [DllImport("gdi32.dll")]
        static extern IntPtr CreateDC(string lpszDriver, string lpszDevice, string lpszOutput, IntPtr lpInitData);
        [DllImport("gdi32.dll", EntryPoint = "DeleteDC")]
        public static extern bool DeleteDC([In] IntPtr hdc);
        [DllImport("gdi32.dll", SetLastError = true)]
        private static extern Int32 GetDeviceCaps(IntPtr hdc, DeviceCap capindex);
        public enum DeviceCap
        {
            /// <summary>
            /// Device driver version
            /// </summary>
            DRIVERVERSION = 0,
            /// <summary>
            /// Device classification
            /// </summary>
            TECHNOLOGY = 2,
            /// <summary>
            /// Horizontal size in millimeters
            /// </summary>
            HORZSIZE = 4,
            /// <summary>
            /// Vertical size in millimeters
            /// </summary>
            VERTSIZE = 6,
            /// <summary>
            /// Horizontal width in pixels
            /// </summary>
            HORZRES = 8,
            /// <summary>
            /// Vertical height in pixels
            /// </summary>
            VERTRES = 10,
            /// <summary>
            /// Number of bits per pixel
            /// </summary>
            BITSPIXEL = 12,
            /// <summary>
            /// Number of planes
            /// </summary>
            PLANES = 14,
            /// <summary>
            /// Number of brushes the device has
            /// </summary>
            NUMBRUSHES = 16,
            /// <summary>
            /// Number of pens the device has
            /// </summary>
            NUMPENS = 18,
            /// <summary>
            /// Number of markers the device has
            /// </summary>
            NUMMARKERS = 20,
            /// <summary>
            /// Number of fonts the device has
            /// </summary>
            NUMFONTS = 22,
            /// <summary>
            /// Number of colors the device supports
            /// </summary>
            NUMCOLORS = 24,
            /// <summary>
            /// Size required for device descriptor
            /// </summary>
            PDEVICESIZE = 26,
            /// <summary>
            /// Curve capabilities
            /// </summary>
            CURVECAPS = 28,
            /// <summary>
            /// Line capabilities
            /// </summary>
            LINECAPS = 30,
            /// <summary>
            /// Polygonal capabilities
            /// </summary>
            POLYGONALCAPS = 32,
            /// <summary>
            /// Text capabilities
            /// </summary>
            TEXTCAPS = 34,
            /// <summary>
            /// Clipping capabilities
            /// </summary>
            CLIPCAPS = 36,
            /// <summary>
            /// Bitblt capabilities
            /// </summary>
            RASTERCAPS = 38,
            /// <summary>
            /// Length of the X leg
            /// </summary>
            ASPECTX = 40,
            /// <summary>
            /// Length of the Y leg
            /// </summary>
            ASPECTY = 42,
            /// <summary>
            /// Length of the hypotenuse
            /// </summary>
            ASPECTXY = 44,
            /// <summary>
            /// Shading and Blending caps
            /// </summary>
            SHADEBLENDCAPS = 45,

            /// <summary>
            /// Logical pixels inch in X
            /// </summary>
            LOGPIXELSX = 88,
            /// <summary>
            /// Logical pixels inch in Y
            /// </summary>
            LOGPIXELSY = 90,

            /// <summary>
            /// Number of entries in physical palette
            /// </summary>
            SIZEPALETTE = 104,
            /// <summary>
            /// Number of reserved entries in palette
            /// </summary>
            NUMRESERVED = 106,
            /// <summary>
            /// Actual color resolution
            /// </summary>
            COLORRES = 108,

            // Printing related DeviceCaps. These replace the appropriate Escapes
            /// <summary>
            /// Physical Width in device units
            /// </summary>
            PHYSICALWIDTH = 110,
            /// <summary>
            /// Physical Height in device units
            /// </summary>
            PHYSICALHEIGHT = 111,
            /// <summary>
            /// Physical Printable Area x margin
            /// </summary>
            PHYSICALOFFSETX = 112,
            /// <summary>
            /// Physical Printable Area y margin
            /// </summary>
            PHYSICALOFFSETY = 113,
            /// <summary>
            /// Scaling factor x
            /// </summary>
            SCALINGFACTORX = 114,
            /// <summary>
            /// Scaling factor y
            /// </summary>
            SCALINGFACTORY = 115,

            /// <summary>
            /// Current vertical refresh rate of the display device (for displays only) in Hz
            /// </summary>
            VREFRESH = 116,
            /// <summary>
            /// Vertical height of entire desktop in pixels
            /// </summary>
            DESKTOPVERTRES = 117,
            /// <summary>
            /// Horizontal width of entire desktop in pixels
            /// </summary>
            DESKTOPHORZRES = 118,
            /// <summary>
            /// Preferred blt alignment
            /// </summary>
            BLTALIGNMENT = 119
        }
        private double GraphicsDpiX
        {
            get
            {
                IntPtr hdc = CreateDC(null, _screen.DeviceName, null, IntPtr.Zero);
                System.Drawing.Graphics gfx = System.Drawing.Graphics.FromHdc(hdc);
                double dpix = gfx.DpiX;
                gfx.Dispose();
                DeleteDC(hdc);
                return dpix;
            }
        }
        private double GraphicsDpiY
        {
            get
            {
                IntPtr hdc = CreateDC(null, _screen.DeviceName, null, IntPtr.Zero);
                System.Drawing.Graphics gfx = System.Drawing.Graphics.FromHdc(hdc);
                double dpiy = gfx.DpiY;
                gfx.Dispose();
                DeleteDC(hdc);
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
                IntPtr hdc = CreateDC("DISPLAY", _screen.DeviceName, null, IntPtr.Zero);
                double dpi = GetDeviceCaps(hdc, DeviceCap.LOGPIXELSX);
                DeleteDC(hdc);
                return dpi;
            }
        }
        private Size DeviceCapsPhysicalSize
        {
            get
            {
                IntPtr hdc = CreateDC("DISPLAY", _screen.DeviceName, null, IntPtr.Zero);
                double w = GetDeviceCaps(hdc, DeviceCap.HORZSIZE);
                double h = GetDeviceCaps(hdc, DeviceCap.VERTSIZE);
                DeleteDC(hdc);
                return new Size(w,h);
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
            double x = Math.Floor(((p.X - PhysicalLocation.X) / PitchX)+0.5) + Bounds.X;
            double y = Math.Floor(((p.Y - PhysicalLocation.Y) / PitchY)+0.5) + Bounds.Y;
            return new Point(x, y);
        }

        public Point PixelToWpf(Point p)
        {
            double x = p.X * PixelToWpfRatioX;
            double y = p.Y * PixelToWpfRatioY;
            return new Point(x,y);
        }

        public Rect PixelToWpf(Rect r)
        {
            return new Rect(PixelToWpf(r.TopLeft), PixelToWpf(r.BottomRight));
        }

        public Point WpfToPixel(Point p)
        {
            double x = Math.Floor((p.X / PixelToWpfRatioX)+0.5);
            double y = Math.Floor((p.Y / PixelToWpfRatioY)+0.5);
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

    }
}
