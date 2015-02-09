using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Interop;

namespace MouseControl
{
    public class Screen
    {
        public event EventHandler PhysicalSizeChanged;
        public event EventHandler PhysicalPositionChanged;

        private static List<Screen> _allScreens = new List<Screen>();
        private static Screen getScreen(System.Windows.Forms.Screen screen)
        {
            Screen wpfScreen = null;
            foreach(Screen s in _allScreens)
            {
                if (s._screen.DeviceName == screen.DeviceName) { wpfScreen = s; break; }
            }
            if (wpfScreen == null)
            {
                wpfScreen = new Screen(screen);
            }

            return wpfScreen;
        }
        // TODO : Microsoft.Win32.SystemEvents.DisplaySettingsChanged
        public static List<Screen> AllScreens
        {
            get
            {
                foreach (System.Windows.Forms.Screen screen in System.Windows.Forms.Screen.AllScreens)
                {
                    getScreen(screen);
                }
                return _allScreens;
            }
        }

        public static Screen getScreen(int nb)
        {
            foreach(Screen s in AllScreens)
            {
                if (s.DeviceName.EndsWith(nb.ToString())) return s;
            }
            return null;
        }

        public static Screen FromPoint(Point point)
        {
            int x = (int)Math.Round(point.X);
            int y = (int)Math.Round(point.Y);

            // are x,y device-independent-pixels ??
            System.Drawing.Point drawingPoint = new System.Drawing.Point(x, y);
            System.Windows.Forms.Screen screen = System.Windows.Forms.Screen.FromPoint(drawingPoint);
            Screen wpfScreen = getScreen(screen);

            return wpfScreen;
        }

        public static Screen PrimaryScreen
        {
            get { return getScreen(System.Windows.Forms.Screen.PrimaryScreen); }
        }

        private System.Windows.Forms.Screen _screen;

        internal Screen(System.Windows.Forms.Screen screen)
        {
            _allScreens.Add(this);
            _screen = screen;
            //DPI = LogPixelSx;

            _pitch_X = Edid.GetSizeForDevID(screen.DeviceName).Width / screen.Bounds.Width;
            _pitch_Y = Edid.GetSizeForDevID(screen.DeviceName).Height / screen.Bounds.Height;

            _physicalLocation = new Point(Bounds.Left * _overallPitch, Bounds.Top * _overallPitch);
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
                _physicalLocation = value;
            }
        }

        public Rect PhysicalBounds
        {
            get
            {
                return new Rect(PhysicalLocation, new Size(Bounds.Width*_pitch_X, Bounds.Height*_pitch_Y));
            }
        }

        public static Rect OverallBounds
        {
            get
            {
                Rect r = Screen.PrimaryScreen.Bounds;
                foreach (Screen s in Screen.AllScreens)
                {
                    r.Union(s.Bounds);
                }
                return r;
            }
        }
        public static Rect PhysicalOverallBounds
        {
            get
            {
                Rect r = Screen.PrimaryScreen.PhysicalBounds;
                foreach (Screen s in Screen.AllScreens)
                {
                    r.Union(s.PhysicalBounds);
                }
                return r;
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
            get { return this._screen.Primary; }
        }

        public string DeviceName
        {
            get { return this._screen.DeviceName; }
        }

        private static double _overallPitch = 25.4/96.0; 
        private double _pitch_X = 25.4 / 96.0;
        public double DpiX
        {
            get { return 25.4/_pitch_X; }
            set {

                _pitch_X = 25.4/value;
                shrinkX();
                if (PhysicalSizeChanged != null)
                    PhysicalSizeChanged(this,new EventArgs());
            }
        }

        private double _pitch_Y = 25.4 / 96.0;
        public double DpiY
        {
            get { return 25.4 / _pitch_Y; }
            set
            {

                _pitch_Y = 25.4 / value;
                //shrinkX();
                if (PhysicalSizeChanged != null)
                    PhysicalSizeChanged(this, new EventArgs());
            }
        }

        public double DpiAvg
        {
            get {
                double pitch = Math.Sqrt((_pitch_X * _pitch_X) + (_pitch_Y * _pitch_Y)) / Math.Sqrt(2);
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


        public static void shrinkX()
        {
            double x = PhysicalOverallBounds.X;

            List<Screen> done = new List<Screen>();

            while (AllScreens.Except(done).Count() > 0)
            {
                bool loop = true;
                while(loop)
                {
                    loop = false;
                    foreach (Screen s in AllScreens.Except(done))
                    {
                        if (s.PhysicalBounds.X<=x)
                        {
                            done.Add(s);
                            if (s.PhysicalBounds.Right > x)
                            {
                                x = s.PhysicalBounds.Right;
                                loop = true;
                            }
                        }
                    }
                }

                Screen minScreen = null;
                foreach (Screen s in AllScreens.Except(done))
                {
                    if (minScreen== null || s.PhysicalBounds.X < minScreen.PhysicalBounds.X)
                    {
                        minScreen = s;
                    }
                }
                if (minScreen!=null)
                {
                    minScreen.PhysicalLocation = new Point(x, minScreen.PhysicalLocation.Y);
                    done.Add(minScreen);
                    x = minScreen.PhysicalBounds.Right;
                }
            }
            // search max continuous X

        }
        public Rect ToUI(Size s)
        {
                Rect all = Screen.PhysicalOverallBounds;

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

        public static Point PhysicalToUI(Size s, Point p)
        {
            Rect all = Screen.PhysicalOverallBounds;

            double ratio = Math.Min(
                s.Width / all.Width,
                s.Height / all.Height
                );

            return new Point(
                (p.X - all.Left) * ratio,
                (p.Y - all.Top) * ratio
                );

        }

        static public Point FromUI(Size s, Point p)
        {
            Rect all = Screen.PhysicalOverallBounds;

            double ratio = Math.Min(
                s.Width / all.Width,
                s.Height / all.Height
                );

            return new Point(
                (p.X/ratio)+all.Left,
                (p.Y/ratio)+all.Top
                );
        }

        public Screen FromPhysicalPoint(Point p)
        {
            foreach(Screen s in AllScreens)
            {
                if (s.PhysicalBounds.Contains(p))
                    return s;
            }
            return null;
        }

        public Point PixelToPhysical(Point p)
        {
            double x = (p.X - Bounds.X) * _pitch_X + PhysicalLocation.X;
            double y = (p.Y - Bounds.Y) * _pitch_Y + PhysicalLocation.Y;
            return new Point(x, y);
        }

        public Point PhysicalToPixel(Point p)
        {
            double x = ((p.X - PhysicalLocation.X) / _pitch_X) + Bounds.X;
            double y = ((p.Y - PhysicalLocation.Y) / _pitch_Y) + Bounds.Y;
            return new Point(x, y);
        }
    }
}
