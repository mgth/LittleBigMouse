using System;
using System.CodeDom;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using LittleBigMouse;

namespace LittleBigMouse
{
    public class AbsoluteRectangle
    {
        public Screen Screen { get; }

        public PixelPoint TopLeft { get; }

        public PixelPoint BottomRight { get; }

        public PixelPoint TopRight => new PixelPoint(TopLeft.Screen,BottomRight.X,TopLeft.Y);
        public PixelPoint BottomLeft => new PixelPoint(TopLeft.Screen, TopLeft.X, BottomRight.Y);

        public AbsoluteRectangle(AbsolutePoint p1, AbsolutePoint p2)
        {
            //if (p1.Screen != p2.Screen) throw new Exception("Points from different configurations");
            Screen = p1.Screen;
            var left = Math.Min(p1.Pixel.X, p2.Pixel.X);
            var right = Math.Max(p1.Pixel.X, p2.Pixel.X);
            var top = Math.Min(p1.Pixel.Y, p2.Pixel.Y);
            var bottom = Math.Max(p1.Pixel.Y, p2.Pixel.Y);

            TopLeft = new PixelPoint(p1.Screen,left,top);
            BottomRight = new PixelPoint(p1.Screen , right, bottom);
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

        protected AbsolutePoint(Screen screen, double x, double y)
        {
            Screen = screen;
            Config = screen.Config;
            X = x;
            Y = y;
        }

        protected AbsolutePoint(ScreenConfig config, double x, double y)
        {
            Config = config;
            X = x;
            Y = y;
        }

        public virtual PhysicalPoint Physical 
            => new PhysicalPoint(Screen,
            Screen.PhysicalLocation.X + (Pixel.X - Screen.PixelLocation.X) * Screen.PitchX,
            Screen.PhysicalLocation.Y + (Pixel.Y - Screen.PixelLocation.Y) * Screen.PitchY
            );

        public abstract PixelPoint Pixel { get; }

        //public virtual WpfPoint Wpf => new WpfPoint(Screen,
        //    Pixel.X*Screen.PixelToWpfRatioX,
        //    Pixel.Y*Screen.PixelToWpfRatioY
        //    );
        public virtual DpiAwarePoint DpiAware  => new DpiAwarePoint(Screen,
            Screen.PixelLocation.X + (Pixel.X - Screen.PixelLocation.X) / Screen.RatioX,
            Screen.PixelLocation.Y + (Pixel.Y - Screen.PixelLocation.Y) / Screen.RatioY
            );

        public PhysicalPoint ToScreen(Screen s)
        {
            return new PhysicalPoint(s, Physical.X, Physical.Y);
        }
    }

    public class PhysicalPoint : AbsolutePoint
    {
        public PhysicalPoint(Screen screen, double x, double y) : base(screen, x, y)
        {
        }
        public PhysicalPoint(ScreenConfig config, double x, double y) : base(config, x, y)
        {
            Screen = TargetScreen;
        }

        public bool Eguals(AbsolutePoint p2)
        {
            if (X != p2?.Physical.X) return false;
            if (Y != p2?.Physical.Y) return false;
            return true;
        }


        public static bool operator ==(PhysicalPoint p1, AbsolutePoint p2)
        {
            if (p1.X != p2?.Physical.X) return false;
            if (p1.Y != p2?.Physical.Y) return false;
            return true;
        }
        public static bool operator !=(PhysicalPoint p1, AbsolutePoint p2)
        {
            if (p1.X != p2?.Physical.X) return true;
            if (p1.Y != p2?.Physical.Y) return true;
            return false;
        }

        public override PhysicalPoint Physical => this;

        public override PixelPoint Pixel => new PixelPoint(Screen,
            Math.Round(Screen.PixelLocation.X + (X - Screen.PhysicalLocation.X) / Screen.PitchX), 
            Math.Round(Screen.PixelLocation.Y + (Y - Screen.PhysicalLocation.Y) / Screen.PitchY)
            );

        public bool IsInside(Screen s = null)
        {
            if (s == null) s = Screen;

            if (X < s.PhysicalLocation.X) return false;
            if (X >= s.PhysicalLocation.X + s.PixelSize.Width * s .PitchX) return false;
            if (Y < s.PhysicalLocation.Y) return false;
            if (Y >= s.PhysicalLocation.Y + s.PixelSize.Height * s.PitchY) return false;
            return true;
        }


        public Screen TargetScreen => Config.AllScreens.FirstOrDefault(IsInside);
    }

    public class PixelPoint : AbsolutePoint
    {
        public PixelPoint(Screen screen, double x, double y) : base(screen, x, y)
        {
        }
        public PixelPoint(ScreenConfig config, double x, double y) : base(config, x, y)
        {
            Screen = TargetScreen;
        }


        public override PixelPoint Pixel => this;


        public bool IsInside(Screen screen = null)
        {
            if (screen == null) screen = Screen;
            if (screen == null) return false;

            if (X < screen.PixelLocation.X) return false;
            if (Y < screen.PixelLocation.Y) return false;
            if (X >= screen.PixelLocation.X + screen.PixelSize.Width) return false;
            if (Y >= screen.PixelLocation.Y + screen.PixelSize.Height) return false;
            return true;
        }

        public PixelPoint Inside
        {
            get
            {
                double x = X;
                double y = Y;

                if (x < Screen.Bounds.TopLeft.Pixel.X) x = Screen.Bounds.TopLeft.Pixel.X;
                else if (x >= Screen.Bounds.BottomRight.Pixel.X) x = Screen.Bounds.BottomRight.Pixel.X - 1;

                if (y < Screen.Bounds.TopLeft.Pixel.Y) y = Screen.Bounds.TopLeft.Pixel.Y;
                else if (y >= Screen.Bounds.BottomRight.Pixel.Y) y = Screen.Bounds.BottomRight.Pixel.Y - 1;

                return new PixelPoint(Screen, Math.Round(x), Math.Round(y));

            }
        }

        public Screen TargetScreen => Config.AllScreens.FirstOrDefault(IsInside);
    }


    public class DpiAwarePoint : AbsolutePoint
    {
        // screenOut.PixelLocation.X + (p.X - screenOut.PixelLocation.X) / (screenOut.DpiX/screenOut.EffectiveDpiX),
        // screenOut.PixelLocation.Y + (p.Y - screenOut.PixelLocation.Y) / (screenOut.DpiY / screenOut.EffectiveDpiY)

        public DpiAwarePoint(Screen screen, double x, double y) : base(screen, x, y)
        {
        }

        public override DpiAwarePoint DpiAware => this;

        public override PixelPoint Pixel => new PixelPoint(Screen,
            Screen.PixelLocation.X + (X - Screen.PixelLocation.X) * Screen.RatioX,
            Screen.PixelLocation.Y + (Y - Screen.PixelLocation.Y) * Screen.RatioY
            );
    }
}


