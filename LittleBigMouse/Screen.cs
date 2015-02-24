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
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Windows;

[assembly: InternalsVisibleTo("ScreenConfig")]
namespace LittleBigMouse
{
    public class Screen
    {
        bool _loaded = false;

        public event EventHandler PhysicalChanged;

        private void OnPhysicalChanged()
        {
            if (PhysicalChanged != null) PhysicalChanged(this, new EventArgs());
        }

        internal System.Windows.Forms.Screen _screen;
        private ScreenConfig _config = null;
        internal Edid _edid;
        // TODO : Microsoft.Win32.SystemEvents.DisplaySettingsChanged

        internal Screen(ScreenConfig config, System.Windows.Forms.Screen screen)
        {
            _config = config;
            _screen = screen;
            _edid = new Edid(screen.DeviceName);
        }

        public int ProductCode
        {
            get { return _edid.ProductCode; }
        }

        public int Serial
        {
            get { return _edid.Serial; }
        }

        public String ID
        {
            get
            {
                //return DeviceName.Substring(4);
                // TODO : It would be better to save actual screen but edid code is broken
                return ProductCode.ToString() + "_" + Serial.ToString() + "_" + Bounds.Width + "x" + Bounds.Height;
            }
        }

        public void Save(RegistryKey baseKey)
        {
            RegistryKey key = baseKey.CreateSubKey(ID);

            key.SetValue("X", PhysicalLocation.X.ToString(), RegistryValueKind.String);
            key.SetValue("Y", PhysicalLocation.Y.ToString(), RegistryValueKind.String);

            key.Close();
        }
        public void Load()
        {
            if (!_loaded)
            {
                RegistryKey key = _config.Key.OpenSubKey(ID);

                Point p = new Point(Config.PhysicalOverallBounds.Right, 0);

                if (key != null)
                {
                    p = new Point(
                        double.Parse(key.GetValue("X", RegistryValueKind.String).ToString()),
                        double.Parse(key.GetValue("Y", RegistryValueKind.String).ToString())
                    );
                    key.Close();
                }

                PhysicalLocation = p;

                _loaded = true;
            }
        }

        public Rect Bounds
        {
            get { return getRect(_screen.Bounds); }
        }

        public Rect InsideBounds
        {
            get { return new Rect(Bounds.TopLeft,new Point(Bounds.BottomRight.X-1.0,Bounds.BottomRight.Y-1.0)); }
        }

        private Point _physicalLocation;
        public Point PhysicalLocation
        {
            get {
                return _physicalLocation;
            }
            set
            {
                // Do not move primary screen but all others
                if(Primary)
                {
                    foreach (Screen s in Config.AllScreens)
                    {
                        if(!s.Primary)
                        {
                            s.PhysicalLocation = new Point
                                (
                                s.PhysicalLocation.X - value.X,
                                s.PhysicalLocation.Y - value.Y
                                );
                        }
                    }
                    _physicalLocation = new Point(0, 0);
                }
                else
                {
                    _physicalLocation = value;           
                }
                OnPhysicalChanged();
            }
        }

        public Rect PhysicalBounds
        {
            get
            {
                return new Rect(PhysicalLocation, PixelToPhysical(InsideBounds.BottomRight));
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

        public double PitchX
        {
            get
            {
                return DeviceCapsPhysicalSize.Width / Bounds.Width;
                //                if (_edid.IsValid) return _edid.PhysicalSize.Width / Bounds.Width;
                //                else return 25.4 / 96.0;
            }
        }
        
        public double DpiX
        {
            get { return 25.4/PitchX; }
        }

        public double PitchY
        {
            get
            {
                return DeviceCapsPhysicalSize.Height / Bounds.Height;
//                if (_edid.IsValid) return _edid.PhysicalSize.Height / Bounds.Height;
//                else return 25.4 / 96.0;
            }
        }
        public double DpiY
        {
            get { return 25.4 / PitchY; }
        }

        public double DpiAvg
        {
            get {
                double pitch = Math.Sqrt((PitchX * PitchX) + (PitchY * PitchY)) / Math.Sqrt(2);
                return 25.4 / pitch;
            }
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

        public double WpfRatioX
        {
            get
            {
                return 96.0 / GraphicsDpiX;
            }
        }
        public double WpfRatioY
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
            double x = Math.Truncate(p.X - Bounds.X) * PitchX + PhysicalLocation.X;
            double y = Math.Truncate(p.Y - Bounds.Y) * PitchY + PhysicalLocation.Y;
            return new Point(x, y);
        }
        public Size PixelToPhysical(Size s)
        {
            double x = Math.Truncate(s.Width) * PitchX + PhysicalLocation.X;
            double y = Math.Truncate(s.Height) * PitchY + PhysicalLocation.Y;
            return new Size(x, y);
        }
        public Point PhysicalToPixel(Point p)
        {
            double x = Math.Truncate((p.X - PhysicalLocation.X) / PitchX) + Bounds.X;
            double y = Math.Truncate((p.Y - PhysicalLocation.Y) / PitchY) + Bounds.Y;
            return new Point(x, y);
        }

        public Point PixelToWpf(Point p)
        {
            double x = Math.Truncate(p.X) * WpfRatioX;
            double y = Math.Truncate(p.Y) * WpfRatioY;
            return new Point(x,y);
        }

        public Rect PixelToWpf(Rect r)
        {
            return new Rect(PixelToWpf(r.TopLeft), PixelToWpf(r.BottomRight));
        }

        public Point WpfToPixel(Point p)
        {
            double x = Math.Truncate(p.X / WpfRatioX);
            double y = Math.Truncate(p.Y / WpfRatioY);
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
