using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Interop;

namespace MouseControl
{
    public class Screen
    {
        public static IEnumerable<Screen> AllScreens
        {
            get
            {
                foreach (System.Windows.Forms.Screen screen in System.Windows.Forms.Screen.AllScreens)
                {
                    yield return new Screen(screen);
                }
            }
        }
        public static Screen GetScreenFrom(Window window)
        {
            WindowInteropHelper windowInteropHelper = new WindowInteropHelper(window);
            System.Windows.Forms.Screen screen = System.Windows.Forms.Screen.FromHandle(windowInteropHelper.Handle);
            Screen wpfScreen = new Screen(screen);
            return wpfScreen;
        }
        public static Screen GetScreen(int nb)
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
            Screen wpfScreen = new Screen(screen);

            return wpfScreen;
        }

        public static Screen PrimaryScreen
        {
            get { return new Screen(System.Windows.Forms.Screen.PrimaryScreen); }
        }

        private readonly System.Windows.Forms.Screen _screen;

        internal Screen(System.Windows.Forms.Screen screen)
        {
            _screen = screen;
        }

        public Rect Bounds
        {
            get { return GetRect(_screen.Bounds); }
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
            get { return this.GetRect(_screen.WorkingArea); }
        }

        private Rect GetRect(System.Drawing.Rectangle value)
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

        private double _dpi;
        public double DPI
        {
            get { return _dpi; }
            set { _dpi = value; }
        }
    }
}
