using Microsoft.Win32;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Interop;

[assembly:InternalsVisibleTo("ScreenConfig")]
namespace MouseControl
{
    public class Screen
    {
        public event EventHandler PhysicalChanged;

        private void OnPhysicalChanged()
        {
            if (PhysicalChanged != null) PhysicalChanged(this, new EventArgs());
        }

        public static RegistryKey RootKey = Registry.CurrentUser.CreateSubKey("SOFTWARE\\" + System.Windows.Forms.Application.CompanyName + "\\" + Application.ResourceAssembly.GetName().Name);

        internal System.Windows.Forms.Screen _screen;
        internal ScreenConfig _config = null;
        internal Edid _edid;
        // TODO : Microsoft.Win32.SystemEvents.DisplaySettingsChanged

        internal Screen(ScreenConfig config, System.Windows.Forms.Screen screen)
        {
            _config = config;
            _screen = screen;
            _edid = new Edid(int.Parse(screen.DeviceName.Substring(11))-1);
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
                return ProductCode.ToString() + "_" + Serial.ToString() + "_" + Bounds.Width + "x" + Bounds.Height;
            }
        }

        public void Save()
        {
            RegistryKey k = RootKey.CreateSubKey(ID);

            k.SetValue("X", PhysicalLocation.X.ToString(), RegistryValueKind.String);
            k.SetValue("Y", PhysicalLocation.Y.ToString(), RegistryValueKind.String);

            k.Close();
        }
        public bool Load()
        {
            RegistryKey k = RootKey.OpenSubKey(ID);

            Point p = new Point(_config.PhysicalOverallBounds.Right, 0);

            if (k != null)
            {
                p = new Point(
                    double.Parse(k.GetValue("X", RegistryValueKind.String).ToString()),
                    double.Parse(k.GetValue("Y", RegistryValueKind.String).ToString())
                );
                k.Close();
            }

            PhysicalLocation = p;

            return true;
        }

        public Rect Bounds
        {
            get { return getRect(_screen.Bounds); }
        }

        private Point _physicalLocation;
        public Point PhysicalLocation
        {
            get { return _physicalLocation; }
            set
            {
                if(Primary)
                {
                    foreach (Screen s in _config.AllScreens)
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
                    OnPhysicalChanged();
                }
            }
        }

        public Rect PhysicalBounds
        {
            get
            {
                return new Rect(PhysicalLocation, new Size(Bounds.Width*PitchX, Bounds.Height*PitchY));
            }
        }


        public Rect WorkingArea
        {
            get { return this.getRect(_screen.WorkingArea); }
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

        public double PitchX
        {
            get
            {
                if (_edid.IsValid) return _edid.PhysicalSize.Width / Bounds.Width;
                else return 25.4 / 96.0;
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
                if (_edid.IsValid) return _edid.PhysicalSize.Height / Bounds.Height;
                else return 25.4 / 96.0;
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
        private static extern Int32 GetDeviceCaps(IntPtr hdc, Int32 capindex);
        private const int LOGPIXELSX = 88;


        public Rect ToUI(Size s)
        {
                Rect all = _config.PhysicalOverallBounds;

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

        public Point PhysicalToPixel(Point p)
        {
            double x = ((p.X - PhysicalLocation.X) / PitchX) + Bounds.X;
            double y = ((p.Y - PhysicalLocation.Y) / PitchY) + Bounds.Y;
            return new Point(x, y);
        }
    }
}
