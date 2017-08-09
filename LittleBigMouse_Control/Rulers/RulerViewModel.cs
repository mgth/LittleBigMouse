using System;
using System.Windows;
using Erp.Notify;
using LbmScreenConfig;

namespace LittleBigMouse_Control.Rulers
{
    public class RulerViewModel : ViewModel
    {
        public RulerViewModel(Screen screen, Screen drawOn, RulerSide side)
        {
            using (this.Suspend())
            {
                Side = side;
                Screen = screen;
                DrawOn = drawOn;                
            }
        }
        public enum RulerSide
        {
            Top,
            Bottom,
            Left,
            Right
        }
        public RulerSide Side
        {
            get => this.Get<RulerSide>(); set => this.Set(value);
        }
        public bool Enabled
        {
            get => this.Get<bool>(); set => this.Set(value);
        }
        public Screen DrawOn
        {
            get => this.Get<Screen>();
            set => this.Set(value);
        }
        public Screen Screen
        {
            get => this.Get<Screen>();
            set => this.Set(value);
        }
        public Visibility Visibility
        {
            get => this.Get<Visibility>(); set => this.Set(value);
        }

        [TriggedOn("Side")]
        public Thickness RulerThickness => this.Get(() =>
        {
            switch (Side)
            {
                case RulerSide.Top:
                    return new Thickness(1, 0, 1, 0);
                case RulerSide.Bottom:
                    return new Thickness(1, 0, 1, 0);
                case RulerSide.Left:
                    return new Thickness(0, 1, 0, 1);
                case RulerSide.Right:
                    return new Thickness(0, 1, 0, 1);
                default:
                    throw new ArgumentOutOfRangeException();
            }
        });

        [TriggedOn(nameof(Enabled))]
        [TriggedOn(nameof(Screen),"BoundsInMm")]
        [TriggedOn(nameof(DrawOn),"BoundsInMm")]
        private void SetVisibility()
        {
            //Visibility = Visibility.Visible;
            //return;
            //if (IsClosed) return;

            if (!Enabled || DrawOn == null || Screen == null)
            {
                Visibility = Visibility.Hidden;
                return;
            }

            switch (Side)
            {
                case RulerSide.Right:
                    var leftTop = new PhysicalPoint(DrawOn.Config, DrawOn, Screen.XLocationInMm, Screen.YLocationInMm);
                    var leftBottom = new PhysicalPoint(DrawOn.Config, DrawOn, Screen.XLocationInMm, Screen.BoundsInMm.Bottom);

                    if (leftBottom.Y <= DrawOn.YLocationInMm || leftTop.Y >= DrawOn.BoundsInMm.Bottom)
                    {
                        Visibility = Visibility.Hidden;
                    }
                    else
                    {
                        Visibility = Visibility.Visible;
                    }
                    break;

                case RulerSide.Left:
                    var rightTop = new PhysicalPoint(DrawOn.Config, DrawOn, Screen.BoundsInMm.Right, Screen.BoundsInMm.Top);
                    var rightBottom = new PhysicalPoint(DrawOn.Config, DrawOn, Screen.BoundsInMm.Right, Screen.BoundsInMm.Bottom);

                    if (rightBottom.Y <= DrawOn.YLocationInMm || rightTop.Y >= DrawOn.BoundsInMm.Bottom)
                    {
                        Visibility = Visibility.Hidden;
                    }
                    else
                    {
                        Visibility = Visibility.Visible;
                    }
                    break;

                case RulerSide.Bottom:
                    PhysicalPoint topLeft = Screen.Bounds.TopLeft.ToScreen(DrawOn);
                    PhysicalPoint topRight = Screen.Bounds.TopRight.ToScreen(DrawOn);

                    if (topRight.X <= DrawOn.XLocationInMm || topLeft.X >= DrawOn.BoundsInMm.Right)
                    {
                        Visibility = Visibility.Hidden;
                    }
                    else
                    {
                        Visibility = Visibility.Visible;
                    }
                    break;
                case RulerSide.Top:
                    PhysicalPoint bottomLeft = Screen.Bounds.BottomLeft.ToScreen(DrawOn);
                    PhysicalPoint bottomRight = Screen.Bounds.BottomRight.ToScreen(DrawOn);

                    if (bottomRight.X <= DrawOn.XLocationInMm || bottomLeft.X >= DrawOn.BoundsInMm.Right)
                    {
                        Visibility = Visibility.Hidden;
                    }
                    else
                    {
                        Visibility = Visibility.Visible;
                    }
                    break;
                default:
                    throw new ArgumentOutOfRangeException();
            }
        }

        [TriggedOn(nameof(Side))]
        [TriggedOn(nameof(DrawOn), "BoundsInMm")]
        [TriggedOn(nameof(WindowHeight))]
        public double WindowTop => this.Get(()
          => (Side == RulerSide.Bottom) ? (DrawOn.Bounds.BottomRight.Dip.Y - WindowHeight) : DrawOn.Bounds.TopLeft.Dip.Y);

        [TriggedOn(nameof(Side))]
        [TriggedOn(nameof(DrawOn), "BoundsInMm")]
        [TriggedOn(nameof(WindowWidth))]
        public double WindowLeft => this.Get(()
          => (Side == RulerSide.Right) ? (DrawOn.Bounds.BottomRight.Dip.X - WindowWidth): DrawOn.Bounds.TopLeft.Dip.X);

        [TriggedOn(nameof(DrawOn), "BoundsInMm")]
        [TriggedOn(nameof(Screen), "BoundsInMm")]
        public double RulerLeft => this.Get(()
          => Screen.Bounds.TopLeft.ToScreen(DrawOn).Dip.X);

        [TriggedOn(nameof(DrawOn), "BoundsInMm")]
        [TriggedOn(nameof(Screen), "BoundsInMm")]
        public double RulerTop => this.Get(()
          => Screen.Bounds.TopLeft.ToScreen(DrawOn).Dip.Y);

        [TriggedOn("Horizontal")]
        [TriggedOn("DrawOn.WidthInDip")]
        [TriggedOn("DrawOn.MmToDipRatioX")]
        public double WindowWidth  => this.Get(() 
               => (Horizontal) ? DrawOn.WidthInDip : (30 * DrawOn.MmToDipRatioX)
            );

        [TriggedOn("Horizontal")]
        [TriggedOn("Screen.BoundsInMm")]
        [TriggedOn(nameof(RulerLeft))]
        [TriggedOn(nameof(WindowWidth))]
        public double RulerWidth => this.Get(()
            => (Horizontal) ? Screen.Bounds.BottomRight.ToScreen(DrawOn).Dip.X - RulerLeft : WindowWidth);
            // => (Horizontal) ? (Screen.Bounds.BottomRight.Pixel.X - Screen.Bounds.TopLeft.Pixel.X)*DrawOn.PixelToDipRatioX : WindowWidth);


        [TriggedOn("DrawOn.Bounds")]
        [TriggedOn(nameof(WindowTop))]
        [TriggedOn("DrawOn.MmToDipRatioY")]
        public double WindowHeight => this.Get(() 
            => (Vertical) ? (DrawOn.Bounds.BottomRight.Pixel.Y - DrawOn.Bounds.TopLeft.Pixel.Y)*DrawOn.PixelToDipRatioY : 30 * DrawOn.MmToDipRatioY);

        [TriggedOn("Vertical")]
        [TriggedOn("Screen.BoundsInMm")]
        [TriggedOn(nameof(RulerTop))]
        [TriggedOn(nameof(WindowHeight))]
        public double RulerHeight => this.Get(()
            => (Vertical) ? Screen.Bounds.BottomRight.ToScreen(DrawOn).Dip.Y - RulerTop : WindowHeight);
        //   => (Vertical) ? (Screen.Bounds.BottomRight.Pixel.Y - Screen.Bounds.TopLeft.Pixel.Y) * DrawOn.PixelToDipRatioY : WindowHeight);

        [TriggedOn(nameof(Screen),"Bounds")]
        [TriggedOn(nameof(Screen),"BoundsInMm")]
        [TriggedOn(nameof(DrawOn),"Bounds")]
        [TriggedOn(nameof(DrawOn), "BoundsInMm")]
        [TriggedOn(nameof(WindowTop))]
        [TriggedOn(nameof(WindowLeft))]
        [TriggedOn(nameof(WindowHeight))]
        [TriggedOn(nameof(RulerTop))]
        [TriggedOn(nameof(RulerLeft))]
        [TriggedOn(nameof(RulerHeight))]
        [TriggedOn(nameof(RulerWidth))]
        public Thickness CanvasMargin => this.Get<Thickness>(()
                
            => Vertical ? new Thickness(
                0,
                Screen.Bounds.TopLeft.ToScreen(DrawOn).Dip.Y - WindowTop,
                0,
                (WindowTop + WindowHeight) - (RulerTop + RulerHeight)
            ) : new Thickness(
                Screen.Bounds.TopLeft.ToScreen(DrawOn).Dip.X - WindowLeft,
                0,
                (WindowLeft + WindowWidth) - (RulerLeft + RulerWidth),
                0
            ));


        [TriggedOn(nameof(Side))]
        public Point GradientStartPoint => this.Get(() =>
        {
            switch (Side)
            {
                case RulerSide.Top: return new Point(0.5, 0);
                case RulerSide.Bottom: return new Point(0.5, 1);
                case RulerSide.Left: return new Point(0, 0.5);
                case RulerSide.Right: return new Point(1, 0.5);
                default:
                    throw new ArgumentOutOfRangeException();
            }
        });

        [TriggedOn(nameof(Side))]
        public Point GradientEndPoint => this.Get(() =>
        {
                switch (Side)
                {
                    case RulerSide.Top: return new Point(0.5, 1);
                    case RulerSide.Bottom: return new Point(0.5, 0);
                    case RulerSide.Left: return new Point(1, 0.5);
                    case RulerSide.Right: return new Point(0, 0.5);
                    default:
                        throw new ArgumentOutOfRangeException();
                }
        });

        [TriggedOn(nameof(Side))]
        public bool Vertical => this.Get(()=>Side == RulerSide.Left) || (Side == RulerSide.Right);

        [TriggedOn(nameof(Side))]
        public bool Horizontal => this.Get(()=>!Vertical);

        [TriggedOn(nameof(Side))]
        public bool Revert => this.Get(() => Side == RulerSide.Right || Side == RulerSide.Bottom);

        [TriggedOn(nameof(Vertical))]
        [TriggedOn(nameof(DrawOn),"WpfToPixelRatioX")]
        [TriggedOn(nameof(DrawOn), "PitchX")]
        [TriggedOn(nameof(DrawOn), "WpfToPixelRatioY")]
        [TriggedOn(nameof(DrawOn), "PitchY")]
        public double SizeRatio => this.Get(() => Vertical ? ((1 / DrawOn.WpfToPixelRatioX) / DrawOn.PitchX) : ((1 / DrawOn.WpfToPixelRatioY) / DrawOn.PitchY));

        [TriggedOn(nameof(Vertical))]
        [TriggedOn(nameof(DrawOn), "WpfToPixelRatioX")]
        [TriggedOn(nameof(DrawOn), "PitchX")]
        [TriggedOn(nameof(DrawOn), "WpfToPixelRatioY")]
        [TriggedOn(nameof(DrawOn), "PitchY")]
        public double LenghtRatio => this.Get(()=>Vertical ? ((1 / DrawOn.WpfToPixelRatioY) / DrawOn.PitchY) : ((1 / DrawOn.WpfToPixelRatioX) / DrawOn.PitchX));


        [TriggedOn(nameof(Vertical))]
        [TriggedOn(nameof(Screen),"HeightInMm")]
        [TriggedOn(nameof(Screen), "WidthInMm")]
        public double Length => this.Get(()=> Vertical ? Screen.HeightInMm : Screen.WidthInMm);

        [TriggedOn(nameof(Vertical))]
        [TriggedOn(nameof(WindowWidth))]
        [TriggedOn(nameof(WindowHeight))]
        public double Width => this.Get(()=> Vertical ? WindowWidth : WindowHeight);

    }
}
