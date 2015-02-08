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

        public static Screen GetScreenFrom(Window window)
        {
            WindowInteropHelper windowInteropHelper = new WindowInteropHelper(window);
            System.Windows.Forms.Screen screen = System.Windows.Forms.Screen.FromHandle(windowInteropHelper.Handle);
            Screen wpfScreen = getScreen(screen);
            return wpfScreen;
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

            _pitch = Edid.GetSizeForDevID(screen.DeviceName).Width / screen.Bounds.Width;

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
                return new Rect(PhysicalLocation, new Size(Bounds.Width*_pitch, Bounds.Height*_pitch));
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

        public Rect LeftRect
        {
            get { return new Rect(OverallBounds.Left, Bounds.Top, Bounds.Left-OverallBounds.Left, Bounds.Height); }
        }
        public Rect RightRect
        {
            get { return new Rect(Bounds.Right, Bounds.Top, OverallBounds.Right-Bounds.Right, Bounds.Height); }
        }
        public Rect TopRect
        {
            get { return new Rect(Bounds.Left, OverallBounds.Top, Bounds.Width, Bounds.Top); }
        }
        public Rect BottomRect
        {
            get { return new Rect(Bounds.Left, Bounds.Bottom, Bounds.Width, OverallBounds.Bottom-Bounds.Bottom); }
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
        private double _pitch = 25.4 / 96.0;
        public double DPI
        {
            get { return 25.4/_pitch; }
            set {

                _pitch = 25.4/value;
                shrinkX();
                if (PhysicalSizeChanged != null)
                    PhysicalSizeChanged(this,new EventArgs());
            }
        }

        [DllImport("gdi32.dll")]
        static extern IntPtr CreateDC(string lpszDriver, string lpszDevice, string lpszOutput, IntPtr lpInitData);
        [DllImport("gdi32.dll", EntryPoint = "DeleteDC")]
        public static extern bool DeleteDC([In] IntPtr hdc);
        [DllImport("gdi32.dll", SetLastError = true)]
        private static extern Int32 GetDeviceCaps(IntPtr hdc, Int32 capindex);
        private const int LOGPIXELSX = 88;


        private double GraphicsDPI
        {
            get
            {
                IntPtr hdc = CreateDC(null, _screen.DeviceName, null, IntPtr.Zero);
                System.Drawing.Graphics gfx = System.Drawing.Graphics.FromHdc(hdc);
                double dpi = gfx.DpiX;
                gfx.Dispose();
                DeleteDC(hdc);
                return dpi;
            }
        }
        private double LogPixelSx
        {
            get
            {
                IntPtr hdc = CreateDC("DISPLAY", _screen.DeviceName, null, IntPtr.Zero);
                double dpi = GetDeviceCaps(hdc, LOGPIXELSX);
                DeleteDC(hdc);
                return dpi;
            }
        }

        

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
    }
}
