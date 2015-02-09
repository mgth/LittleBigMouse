using MouseKeyboardActivityMonitor;
using MouseKeyboardActivityMonitor.WinApi;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;

namespace MouseControl
{
    public class ScreenConfig
    {
        public event EventHandler RegistryChanged;

        private List<Screen> _allScreens = new List<Screen>();
        private readonly MouseHookListener _MouseHookManager = new MouseHookListener(new GlobalHooker());

        private Screen _currentScreen=null;
        private Point _oldPoint;

        public void Enable()
        {
            _MouseHookManager.MouseMoveExt += _MouseHookManager_MouseMoveExt;
            _MouseHookManager.Enabled = true;
        }
        public void Disable()
        {
            _MouseHookManager.MouseMoveExt -= _MouseHookManager_MouseMoveExt;
            _MouseHookManager.Enabled = false;
        }
        private void _MouseHookManager_MouseMoveExt(object sender, MouseEventExtArgs e)
        {
            // TODO : remove
            //labelX.Content = e.X;
            //labelY.Content = e.Y;

            Point pIn = new Point(e.X, e.Y);

            if (_currentScreen == null) _currentScreen = FromPoint(pIn);

            if (_currentScreen.Bounds.Contains(pIn))
            {
                _oldPoint = pIn;
                return;
            }

            Point pOutPhysical = _currentScreen.PixelToPhysical(pIn);

            Screen screenOut = FromPhysicalPoint(pOutPhysical);

            if (screenOut != null)
            {
                Point pOut = screenOut.PhysicalToPixel(pOutPhysical);

                Mouse.SetCursorPos((int)pOut.X, (int)pOut.Y);

                _currentScreen = screenOut;

                if (_currentScreen.DpiAvg > 110)
                {
                    if (_currentScreen.DpiAvg > 138)
                        Mouse.setCursorAero(3);
                    else Mouse.setCursorAero(2);
                }
                else Mouse.setCursorAero(1);

                Mouse.MouseSpeed = Math.Round((5.0 / 96.0) * _currentScreen.DpiAvg, 0);

                _oldPoint = pIn;
                e.Handled = true;
            }
            else
            {
                Mouse.SetCursorPos((int)_oldPoint.X, (int)_oldPoint.Y);
            }

        }


        public List<Screen> AllScreens
        {
            get
            {
                return _allScreens;
            }
        }

        public static ScreenConfig Load()
        {
            ScreenConfig config = new ScreenConfig();

            foreach (System.Windows.Forms.Screen screen in System.Windows.Forms.Screen.AllScreens)
            {
                config.getScreen(screen);
            }
            return config;
        }

        public void Save()
        {
            foreach (Screen s in AllScreens)
                s.Save();

            if (RegistryChanged != null) RegistryChanged(this, new EventArgs());
        }

        private Screen getScreen(System.Windows.Forms.Screen screen)
        {
            Screen wpfScreen = null;
            foreach(Screen s in AllScreens)
            {
                if (s._screen.DeviceName == screen.DeviceName) { wpfScreen = s; break; }
            }
            if (wpfScreen == null)
            {
                wpfScreen = new Screen(this,screen);

                wpfScreen.Load();

               AllScreens.Add(wpfScreen);
            }
            return wpfScreen;
        }

        public Screen getScreen(int nb)
        {
            foreach (Screen s in AllScreens)
            {
                if (s.DeviceName.EndsWith(nb.ToString())) return s;
            }
            return null;
        }
        public Screen FromPoint(Point point)
        {
            int x = (int)Math.Round(point.X);
            int y = (int)Math.Round(point.Y);

            // are x,y device-independent-pixels ??
            System.Drawing.Point drawingPoint = new System.Drawing.Point(x, y);
            System.Windows.Forms.Screen screen = System.Windows.Forms.Screen.FromPoint(drawingPoint);
            Screen wpfScreen = getScreen(screen);

            return wpfScreen;
        }
        public Screen PrimaryScreen
        {
            get { return getScreen(System.Windows.Forms.Screen.PrimaryScreen); }
        }

// Original windows locations
        public Rect OverallBounds
        {
            get
            {
                Rect r = PrimaryScreen.Bounds;
                foreach (Screen s in AllScreens)
                {
                    r.Union(s.Bounds);
                }
                return r;
            }
        }

        // Physical Locations
        public Rect PhysicalOverallBounds
        {
            get
            {
                Rect r = new Rect();
                foreach (Screen s in AllScreens)
                {
                    if (r.Width == 0)
                        r = s.PhysicalBounds;
                    else
                        r.Union(s.PhysicalBounds);
                }
                return r;
            }
        }

        public Point PhysicalToUI(Size s, Point p)
        {
            Rect all = PhysicalOverallBounds;

            double ratio = Math.Min(
                s.Width / all.Width,
                s.Height / all.Height
                );

            return new Point(
                (p.X - all.Left) * ratio,
                (p.Y - all.Top) * ratio
                );
        }

        public Point FromUI(Size s, Point p)
        {
            Rect all = PhysicalOverallBounds;

            double ratio = Math.Min(
                s.Width / all.Width,
                s.Height / all.Height
                );

            return new Point(
                (p.X / ratio) + all.Left,
                (p.Y / ratio) + all.Top
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

    }
}
