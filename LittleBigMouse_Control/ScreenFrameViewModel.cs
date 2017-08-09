using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Animation;
using Erp.Notify;

namespace LittleBigMouse_Control
{
    internal class ScreenFrameViewModel : ScreenViewModel
    {
        public override Type ViewType => typeof(ScreenFrameView);

        public ScreenControlViewModel ControlViewModel
        {
            get => this.Get<ScreenControlViewModel>(); set => this.Set(value);
        }

         public MultiScreensViewModel Presenter
        {
            get => this.Get<MultiScreensViewModel>(); set => this.Set(value);
        }

        public double Width
        {
            get => this.Get<double>(); private set => this.Set(value);
        }

        public double Height
        {
            get => this.Get<double>(); private set => this.Set(value);
        }

        [TriggedOn("Screen.OutsideBoundsInMm")]
        [TriggedOn("Presenter.Ratio")]
        private void UpdateWidth() =>
            Width = (Screen?.OutsideBoundsInMm.Width??0) * (Presenter?.Ratio??1);

        [TriggedOn("Screen.OutsideBoundsInMm")]
        [TriggedOn("Presenter.Ratio")]
        private void UpdateHeight() =>
            Height = (Screen?.OutsideBoundsInMm.Height??0) * (Presenter?.Ratio??1);

        //public Thickness Margin => new Thickness(
        //    Presenter.PhysicalToUiX(Screen.OutsideBoundsInMm.X),
        //    Presenter.PhysicalToUiY(Screen.OutsideBoundsInMm.Y),
        //    0, 0);

        public Thickness Margin
        {
            get => this.Get<Thickness>(); private set => this.Set(value);
        }

        [TriggedOn("Presenter.Size")]
        [TriggedOn("Screen.Config.MovingPhysicalOutsideBounds")]
        [TriggedOn("Screen.OutsideBoundsInMm")]
        [TriggedOn("Presenter.Ratio")]
        private void UpdateMargin()
        {
            if (Presenter == null) return;

            double x = Presenter.PhysicalToUiX(Screen.OutsideBoundsInMm.X);
            double y = Presenter.PhysicalToUiY(Screen.OutsideBoundsInMm.Y);

            Margin = new Thickness(x,y,0,0);
        }


        [TriggedOn("Presenter.Ratio")]
        public Thickness LogoPadding => new Thickness(4 * Presenter.Ratio);

        [TriggedOn("Presenter.Ratio")]
        private double Ratio => Presenter?.Ratio ?? 1;

        [TriggedOn("Screen.TopBorder")]
        [TriggedOn("Presenter.Ratio")]
        public GridLength TopBorder => new GridLength((Screen?.TopBorder??0) * Ratio);
        [TriggedOn("Screen.BottomBorder")]
        [TriggedOn("Presenter.Ratio")]
        public GridLength BottomBorder => new GridLength((Screen?.BottomBorder??0) * Ratio);

        [TriggedOn("Screen.LeftBorder")]
        [TriggedOn("Presenter.Ratio")]
        public GridLength LeftBorder => new GridLength((Screen?.LeftBorder??0) * Ratio);

        [TriggedOn("Screen.RightBorder")]
        [TriggedOn("Presenter.Ratio")]
        public GridLength RightBorder => new GridLength((Screen?.RightBorder??0) * Ratio);

        #region Unrotated

        public double UnrotatedWidth
        {
            get => this.Get<double>(); private set => this.Set(value);
        }

        public double UnrotatedHeight
        {
            get => this.Get<double>(); private set => this.Set(value);
        }

        [TriggedOn("Screen")]
        [TriggedOn("Screen.Monitor.DisplayOrientation")]
        [TriggedOn("Width")]
        [TriggedOn("Height")]
        private void UpdateUnrotatedWidthHeight()
        {
            if(Screen?.Monitor == null) return;

            if (Screen.Monitor.DisplayOrientation % 2 == 0)
            {
                UnrotatedHeight = Height;
                UnrotatedWidth = Width;
            }
            else
            {
                UnrotatedHeight = Width;
                UnrotatedWidth = Height;
            }
        }

        public GridLength UnrotatedTopBorder
        {
            get => this.Get<GridLength>(); private set => this.Set(value);
        }

        public GridLength UnrotatedRightBorder
        {
            get => this.Get<GridLength>(); private set => this.Set(value);
        }

        public GridLength UnrotatedBottomBorder
        {
            get => this.Get<GridLength>(); private set => this.Set(value);
        }

        public GridLength UnrotatedLeftBorder
        {
            get => this.Get<GridLength>(); private set => this.Set(value);
        }

        [TriggedOn(nameof(LeftBorder))]
        [TriggedOn(nameof(TopBorder))]
        [TriggedOn(nameof(RightBorder))]
        [TriggedOn(nameof(BottomBorder))]
        private void UpdateUnrotated()
        {
            if (Screen == null) return;

            GridLength[] unrotated = { TopBorder, RightBorder, BottomBorder, LeftBorder };

            int o = Screen.Monitor.DisplayOrientation;

            UnrotatedTopBorder = unrotated[o++ % 4];
            UnrotatedRightBorder = unrotated[o++ % 4];
            UnrotatedBottomBorder = unrotated[o++ % 4];
            UnrotatedLeftBorder = unrotated[o % 4];
        }
        #endregion

        [TriggedOn("Presenter.ScreenControlGetter")]
        [TriggedOn(nameof(Screen))]
        private void UpdateScreenGuiControl()
        {
            ControlViewModel 
                = Presenter?.ScreenControlGetter?.GetScreenControlViewModel(Screen);

            if(ControlViewModel!=null)
            ControlViewModel.Frame = this;
        }


        public Transform ScreenOrientation
        {
            get => this.Get<Transform>(); private set => this.Set(value);
        }

        [TriggedOn("Screen.Monitor.DisplayOrientation")]
        [TriggedOn(nameof(Width))]
        [TriggedOn(nameof(Height))]
        public void UpddateScreenOrientation()
        {
            if (Screen?.Monitor == null) return;

                var t = new TransformGroup();
                if (Screen.Monitor.DisplayOrientation > 0) t.Children.Add(new RotateTransform(90 * Screen.Monitor.DisplayOrientation));

                switch (Screen.Monitor.DisplayOrientation)
                {
                    case 1:
                        t.Children.Add(new TranslateTransform(Width, 0));
                        break;
                    case 2:
                        t.Children.Add(new TranslateTransform(Width, Height));
                        break;
                    case 3:
                        t.Children.Add(new TranslateTransform(0, Height));
                        break;
                }

            ScreenOrientation = t;      
        }

        private Viewbox _logo;

        public Viewbox Logo
        {
            get => this.Get<Viewbox>(); private set => this.Set(value);
        }

        [TriggedOn("Screen.Monitor.ManufacturerCode")]
        public void UpdateLogo()
        {
            if (Screen?.Monitor == null) return;

                switch (Screen.Monitor.ManufacturerCode.ToLower())
                {
                    case "sam":
                    Logo = (Viewbox)Application.Current.FindResource("LogoSam");
                        return;
                    case "del":
                    Logo = (Viewbox)Application.Current.FindResource("LogoDel"); return;
                case "api":
                    Logo = (Viewbox)Application.Current.FindResource("LogoAcer"); return;
                case "atk":
                    Logo = (Viewbox)Application.Current.FindResource("LogoAsus"); return;
                case "eiz":
                    Logo = (Viewbox)Application.Current.FindResource("LogoEizo"); return;
                case "ben":
                    case "bnq":
                    Logo = (Viewbox)Application.Current.FindResource("LogoBenq"); return;
                case "nec":
                    Logo = (Viewbox)Application.Current.FindResource("LogoNec"); return;
                case "hpq":
                    Logo = (Viewbox)Application.Current.FindResource("LogoHp"); return;
                case "lg":
                    Logo = (Viewbox)Application.Current.FindResource("LogoLg"); return;
                case "Apl":
                    Logo = (Viewbox)Application.Current.FindResource("LogoApple"); return;
                case "fuj":
                    Logo = (Viewbox)Application.Current.FindResource("LogoFujitsu"); return;
                case "ibm":
                    Logo = (Viewbox)Application.Current.FindResource("LogoIbm"); return;
                case "mat":
                    Logo = (Viewbox)Application.Current.FindResource("LogoPanasonic"); return;
                case "sny":
                    Logo = (Viewbox)Application.Current.FindResource("LogoSony"); return;
                case "tos":
                    Logo = (Viewbox)Application.Current.FindResource("LogoToshiba"); return;
                case "aoc":
                    Logo = (Viewbox)Application.Current.FindResource("LogoAoc"); return;
                case "ivm":
                    Logo = (Viewbox)Application.Current.FindResource("LogoIiyama"); return;
                case "len":
                    Logo = (Viewbox)Application.Current.FindResource("LogoLenovo"); return;
                case "phl":
                    Logo = (Viewbox)Application.Current.FindResource("LogoPhilips"); return;
                case "hei":
                    Logo = (Viewbox)Application.Current.FindResource("LogoYundai"); return;
                case "cpq":
                    Logo = (Viewbox)Application.Current.FindResource("LogoCompaq"); return;
                case "htc":
                    Logo = (Viewbox)Application.Current.FindResource("LogoHitachi"); return;
                case "hyo":
                    Logo = (Viewbox)Application.Current.FindResource("LogoQnix"); return;
                case "nts":
                    Logo = (Viewbox)Application.Current.FindResource("LogoIolair"); return;
                case "otm":
                    Logo = (Viewbox)Application.Current.FindResource("LogoOptoma"); return;
                default:
                    Logo = (Viewbox)Application.Current.FindResource("LogoLbm"); return;
            }
            }
        }

    
}


