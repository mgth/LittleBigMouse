using System;
using System.Diagnostics;
using System.Linq;
using System.Windows;
using WinAPI;

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
            if (p.Mm.X < TopLeft.Mm.X) return false;
            if (p.Mm.X > BottomRight.Mm.X) return false;
            if (p.Mm.Y < TopLeft.Mm.Y) return false;
            if (p.Mm.Y > BottomLeft.Mm.Y) return false;
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


        public virtual PhysicalPoint Mm 
            => new PhysicalPoint(Config, Screen,
            Screen.LocationInMm.X + (Pixel.X - Screen.LocationInPixels.X) * Screen.PitchX,
            Screen.LocationInMm.Y + (Pixel.Y - Screen.LocationInPixels.Y) * Screen.PitchY
            );

        public abstract PixelPoint Pixel { get; }

        public virtual DipPoint Dip
        {
            get
            {
                var aw = NativeMethods.GetThreadDpiAwarenessContext();

                switch (aw)
                {
                    case NativeMethods.DPI_Awareness_Context.Per_Monitor_Aware:
                        return new DipPoint(Config, Screen,
                            Pixel.X * Screen.PixelToDipRatioX,
                            Pixel.Y * Screen.PixelToDipRatioY
                            );
                    case NativeMethods.DPI_Awareness_Context.Per_Monitor_Aware_V2:
                        return new DipPoint(Config, Screen,
                        //Screen.LocationInPixels.X + (Pixel.X - Screen.LocationInPixels.X) * Config.PrimaryScreen.PixelToDipRatioX,
                        //Screen.LocationInPixels.Y + (Pixel.Y - Screen.LocationInPixels.Y) * Config.PrimaryScreen.PixelToDipRatioY
                            Pixel.X * Screen.PixelToDipRatioX,
                            Pixel.Y * Screen.PixelToDipRatioY
                            );
                    case NativeMethods.DPI_Awareness_Context.System_Aware:
                        return new DipPoint(Config, Screen,
                            Pixel.X * Screen.PixelToDipRatioX,
                            Pixel.Y * Screen.PixelToDipRatioY
                            );
                    case NativeMethods.DPI_Awareness_Context.Unaware:
                    default:
                        return new DipPoint(Config, Screen,
                            Screen.LocationInPixels.X + (Pixel.X - Screen.LocationInPixels.X) * Screen.PixelToDipRatioX,
                            Screen.LocationInPixels.Y + (Pixel.Y - Screen.LocationInPixels.Y) * Screen.PixelToDipRatioY
                            );
                }
            }
        }

        public virtual DipPoint MouseDip => new DipPoint(Config, Screen,
            Screen.LocationInPixels.X + (Pixel.X - Screen.LocationInPixels.X)/*/Screen.WpfToPixelRatioX*/,
            Screen.LocationInPixels.Y + (Pixel.Y - Screen.LocationInPixels.Y)/*/Screen.WpfToPixelRatioY*/
        );

        public PhysicalPoint ToScreen(Screen s)
        {
            return new PhysicalPoint(Config, s, Mm.X, Mm.Y);
        }

        public abstract bool IsInside(Screen screen = null);
        public Screen TargetScreen
        {
            get
            {
                var s = Config.AllScreens.FirstOrDefault(IsInside);
                if (s == null)
                {
                    
                }
                return s;
            }
        }
    }

    public class PhysicalPoint : AbsolutePoint
    {
        public PhysicalPoint(ScreenConfig config, Screen screen, double x, double y) : base(config, screen, x, y)
        {
            if (Screen==null) Screen = TargetScreen;
        }

        public bool Eguals(AbsolutePoint p2)
        {
            if (p2?.Mm == null || Math.Abs(X - p2.Mm.X) > double.Epsilon) return false;
            return Equals(Y, p2.Mm.Y);
        }


        public override PhysicalPoint Mm => this;

        public override PixelPoint Pixel => new PixelPoint(Config, Screen,
            Math.Round(Screen.LocationInPixels.X + (X - Screen.LocationInMm.X) / Screen.PitchX), 
            Math.Round(Screen.LocationInPixels.Y + (Y - Screen.LocationInMm.Y) / Screen.PitchY)
            );

        public override bool IsInside(Screen s = null)
        {
            if (s == null) s = Screen;
            if (s == null) return false;

            if (X < s.LocationInMm.X) return false;
            if (X >= s.LocationInMm.X + s.SizeInPixels.Width * s .PitchX) return false;
            if (Y < s.LocationInMm.Y) return false;
            if (Y >= s.LocationInMm.Y + s.SizeInPixels.Height * s.PitchY) return false;
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
                (X < screen.LocationInPixels.X) ||
                (Y < screen.LocationInPixels.Y) ||
                (X >= screen.LocationInPixels.X + screen.SizeInPixels.Width) ||
                (Y >= screen.LocationInPixels.Y + screen.SizeInPixels.Height))
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

                if (x < Screen.BoundsInPixels.X) x = Screen.BoundsInPixels.X;
                else if (x >= Screen.BoundsInPixels.Right) x = Screen.BoundsInPixels.Right - 1;

                if (y < Screen.BoundsInPixels.Y) y = Screen.BoundsInPixels.Y;
                else if (y >= Screen.BoundsInPixels.Bottom) y = Screen.BoundsInPixels.Bottom - 1;

                return new PixelPoint(Config, Screen, Math.Round(x), Math.Round(y));

            }
        }

    }


    public class DipPoint : AbsolutePoint
    {
        // screenOut.LocationInPixels.X + (p.X - screenOut.LocationInPixels.X) / (screenOut.DpiX/screenOut.EffectiveDpiX),
        // screenOut.LocationInPixels.Y + (p.Y - screenOut.LocationInPixels.Y) / (screenOut.DpiY / screenOut.EffectiveDpiY)

        public DipPoint(ScreenConfig config, Screen screen, double x, double y) : base(config, screen, x, y)
        {
        }

        public override DipPoint Dip => this;

        public override PixelPoint Pixel
        {
            get
            {
                var aw = NativeMethods.GetThreadDpiAwarenessContext();

                switch (aw)
                {
                   case NativeMethods.DPI_Awareness_Context.Per_Monitor_Aware:
                    case NativeMethods.DPI_Awareness_Context.System_Aware:
                        return new PixelPoint(Config, Screen,
                            X * Screen.WpfToPixelRatioX,
                            Y * Screen.WpfToPixelRatioY
                            );
                    default:
                        return new PixelPoint(Config, Screen,
                            Screen.LocationInPixels.X + (X - Screen.LocationInPixels.X) * Screen.WpfToPixelRatioX,
                            Screen.LocationInPixels.Y + (Y - Screen.LocationInPixels.Y) * Screen.WpfToPixelRatioY
                            );
                }
            }
        }
 
    public override bool IsInside(Screen screen = null)
        {
            if (screen == null) screen = Screen;
            if (screen == null) return false;

            DipPoint topleft = screen.Bounds.TopLeft.Dip;
            DipPoint bottomRight = screen.Bounds.BottomRight.Dip;

            if (X < topleft.X) return false;
            if (Y < topleft.Y) return false;
            if (X >= bottomRight.X) return false;
            if (Y >= bottomRight.Y) return false;
            return true;
        }
    }
}


