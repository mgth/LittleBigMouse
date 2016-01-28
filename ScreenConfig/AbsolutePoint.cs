using System;
using System.Diagnostics;
using System.Linq;
using System.Windows;
using WinAPI_User32;

namespace LbmScreenConfig
{
    public class AbsoluteRectangle
    {
        public Screen Screen { get; }

        public PixelPoint TopLeft { get; }

        public PixelPoint BottomRight { get; }

        public PixelPoint TopRight => new PixelPoint(Screen.Config, TopLeft.Screen,BottomRight.X,TopLeft.Y);
        public PixelPoint BottomLeft => new PixelPoint(Screen.Config, TopLeft.Screen, TopLeft.X, BottomRight.Y);

        public AbsoluteRectangle(AbsolutePoint p1, AbsolutePoint p2)
        {
            //if (p1.Screen != p2.Screen) throw new Exception("Points from different configurations");
            Screen = p1.Screen;
            var left = Math.Min(p1.Pixel.X, p2.Pixel.X);
            var right = Math.Max(p1.Pixel.X, p2.Pixel.X);
            var top = Math.Min(p1.Pixel.Y, p2.Pixel.Y);
            var bottom = Math.Max(p1.Pixel.Y, p2.Pixel.Y);

            TopLeft = new PixelPoint(p1.Config, p1.Screen,left,top);
            BottomRight = new PixelPoint(p1.Config, p1.Screen , right, bottom);
        }

        public bool Contains(AbsolutePoint p)
        {
            if (p.Physical.X < TopLeft.Physical.X) return false;
            if (p.Physical.X > BottomRight.Physical.X) return false;
            if (p.Physical.Y < TopLeft.Physical.Y) return false;
            if (p.Physical.Y > BottomLeft.Physical.Y) return false;
            return true;
        }
    }

    public abstract class AbsolutePoint
    {
        public double X { get; }
        public double Y { get; }
        public ScreenConfig Config { get; } 
        public Screen Screen { get; protected set; }

        public Point Point => new Point(X,Y); 

        protected AbsolutePoint(ScreenConfig config, Screen screen, double x, double y)
        {
            X = x;
            Y = y;
            Config = config;
            Screen = screen??TargetScreen;
        }


        public virtual PhysicalPoint Physical 
            => new PhysicalPoint(Config, Screen,
            Screen.PhysicalLocation.X + (Pixel.X - Screen.PixelLocation.X) * Screen.PitchX,
            Screen.PhysicalLocation.Y + (Pixel.Y - Screen.PixelLocation.Y) * Screen.PitchY
            );

        public abstract PixelPoint Pixel { get; }

        public virtual WpfPoint Wpf
        {
            get
            {
                Process p = Process.GetCurrentProcess();

                Process_DPI_Awareness aw = Process_DPI_Awareness.Process_Per_Monitor_DPI_Aware;

                User32.GetProcessDpiAwareness(p.Handle,out aw);

                switch (aw)
                {
                    case Process_DPI_Awareness.Process_Per_Monitor_DPI_Aware:
                        return new WpfPoint(Config, Screen,
                            Pixel.X * Screen.PixelToWpfRatioX,
                            Pixel.Y * Screen.PixelToWpfRatioY
                            );
                    case Process_DPI_Awareness.Process_System_DPI_Aware:
                        return new WpfPoint(Config, Screen,
                            Pixel.X*Screen.PixelToWpfRatioX,
                            Pixel.Y*Screen.PixelToWpfRatioY
                            );
                    default:
                        return new WpfPoint(Config, Screen,
                            Screen.PixelLocation.X + (Pixel.X - Screen.PixelLocation.X) * Screen.PixelToWpfRatioX,
                            Screen.PixelLocation.Y + (Pixel.Y - Screen.PixelLocation.Y) * Screen.PixelToWpfRatioY
                            );
                }
            }
        }

        public virtual WpfPoint MouseWpf
        {
            get
            {
                return new WpfPoint(Config, Screen,
                    Screen.PixelLocation.X + (Pixel.X - Screen.PixelLocation.X)/*/Screen.WpfToPixelRatioX*/,
                    Screen.PixelLocation.Y + (Pixel.Y - Screen.PixelLocation.Y)/*/Screen.WpfToPixelRatioY*/
                    );
            }
        }

        public PhysicalPoint ToScreen(Screen s)
        {
            return new PhysicalPoint(Config, s, Physical.X, Physical.Y);
        }

        public abstract bool IsInside(Screen screen = null);
        public Screen TargetScreen => Config.AllScreens.FirstOrDefault(IsInside);
    }

    public class PhysicalPoint : AbsolutePoint
    {
        public PhysicalPoint(ScreenConfig config, Screen screen, double x, double y) : base(config, screen, x, y)
        {
            if (Screen==null) Screen = TargetScreen;
        }

        public bool Eguals(AbsolutePoint p2)
        {
            if (X != p2?.Physical.X) return false;
            return Y == p2?.Physical.Y;
        }


        public override PhysicalPoint Physical => this;

        public override PixelPoint Pixel => new PixelPoint(Config, Screen,
            Math.Round(Screen.PixelLocation.X + (X - Screen.PhysicalLocation.X) / Screen.PitchX), 
            Math.Round(Screen.PixelLocation.Y + (Y - Screen.PhysicalLocation.Y) / Screen.PitchY)
            );

        public override bool IsInside(Screen s = null)
        {
            if (s == null) s = Screen;
            if (s == null) return false;

            if (X < s.PhysicalLocation.X) return false;
            if (X >= s.PhysicalLocation.X + s.PixelSize.Width * s .PitchX) return false;
            if (Y < s.PhysicalLocation.Y) return false;
            if (Y >= s.PhysicalLocation.Y + s.PixelSize.Height * s.PitchY) return false;
            return true;
        }
    }

    public class PixelPoint : AbsolutePoint
    {
        public PixelPoint(ScreenConfig config, Screen screen, double x, double y) : base(config, screen, x, y)
        {
            
        }


        public override PixelPoint Pixel => this;


        public override bool IsInside(Screen screen = null)
        {
            if (screen == null) screen = Screen;
            if (screen == null) return false;

            if (
                (X < screen.PixelLocation.X) ||
                (Y < screen.PixelLocation.Y) ||
                (X >= screen.PixelLocation.X + screen.PixelSize.Width) ||
                (Y >= screen.PixelLocation.Y + screen.PixelSize.Height))
            {

                return false;
            }
            return true;
        }

        public PixelPoint Inside
        {
            get
            {
                double x = X;
                double y = Y;

                if (x < Screen.PixelBounds.X) x = Screen.PixelBounds.X;
                else if (x >= Screen.PixelBounds.Right) x = Screen.PixelBounds.Right - 1;

                if (y < Screen.PixelBounds.Y) y = Screen.PixelBounds.Y;
                else if (y >= Screen.PixelBounds.Bottom) y = Screen.PixelBounds.Bottom - 1;

                return new PixelPoint(Config, Screen, Math.Round(x), Math.Round(y));

            }
        }

    }


    public class WpfPoint : AbsolutePoint
    {
        // screenOut.PixelLocation.X + (p.X - screenOut.PixelLocation.X) / (screenOut.DpiX/screenOut.EffectiveDpiX),
        // screenOut.PixelLocation.Y + (p.Y - screenOut.PixelLocation.Y) / (screenOut.DpiY / screenOut.EffectiveDpiY)

        public WpfPoint(ScreenConfig config, Screen screen, double x, double y) : base(config, screen, x, y)
        {
        }

        public override WpfPoint Wpf => this;

        public override PixelPoint Pixel
        {
            get
            {
                Process p = Process.GetCurrentProcess();

                Process_DPI_Awareness aw = Process_DPI_Awareness.Process_Per_Monitor_DPI_Aware;

                User32.GetProcessDpiAwareness(p.Handle, out aw);

                switch (aw)
                {
                   case Process_DPI_Awareness.Process_Per_Monitor_DPI_Aware:
                    case Process_DPI_Awareness.Process_System_DPI_Aware:
                        return new PixelPoint(Config, Screen,
                            X * Screen.WpfToPixelRatioX,
                            Y * Screen.WpfToPixelRatioY
                            );
                    default:
                        return new PixelPoint(Config, Screen,
                            Screen.PixelLocation.X + (X - Screen.PixelLocation.X) * Screen.WpfToPixelRatioX,
                            Screen.PixelLocation.Y + (Y - Screen.PixelLocation.Y) * Screen.WpfToPixelRatioY
                            );
                }
            }
        }
 
    public override bool IsInside(Screen screen = null)
        {
            if (screen == null) screen = Screen;
            if (screen == null) return false;

            WpfPoint topleft = screen.Bounds.TopLeft.Wpf;
            WpfPoint bottomRight = screen.Bounds.BottomRight.Wpf;

            if (X < topleft.X) return false;
            if (Y < topleft.Y) return false;
            if (X >= bottomRight.X) return false;
            if (Y >= bottomRight.Y) return false;
            return true;
        }
    }
}


