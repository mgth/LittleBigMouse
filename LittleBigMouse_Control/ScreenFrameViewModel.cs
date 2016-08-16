using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Animation;
using NotifyChange;

namespace LittleBigMouse_Control
{
    internal class ScreenFrameViewModel : ScreenViewModel
    {
        public override Type ViewType => typeof(ScreenFrameView);

        public ScreenControlViewModel ControlViewModel
        {
            get { return GetProperty<ScreenControlViewModel>(); }
            set { SetAndWatch(value); }
        }

         public MultiScreensViewModel Presenter
        {
            get { return GetProperty<MultiScreensViewModel>(); }
            set { SetAndWatch(value); }
        }

        public double Width
        {
            get { return GetProperty<double>(); }
            private set { SetProperty(value); }
        }

        public double Height
        {
            get { return GetProperty<double>(); }
            private set { SetProperty(value); }
        }

        [DependsOn("Screen.PhysicalOutsideBounds", "Presenter.Ratio")]
        private void UpdateWidth() =>
            Width = (Screen?.PhysicalOutsideBounds.Width??0) * (Presenter?.Ratio??1);

        [DependsOn("Screen.PhysicalOutsideBounds", "Presenter.Ratio")]
        private void UpdateHeight() =>
            Height = (Screen?.PhysicalOutsideBounds.Height??0) * (Presenter?.Ratio??1);

        //public Thickness Margin => new Thickness(
        //    Presenter.PhysicalToUiX(Screen.PhysicalOutsideBounds.X),
        //    Presenter.PhysicalToUiY(Screen.PhysicalOutsideBounds.Y),
        //    0, 0);

        public Thickness Margin
        {
            get { return GetProperty<Thickness>(); }
            private set { SetProperty(value); }
        }

        [DependsOn(
            "Presenter.Size",
            //"Screen.PhysicalX",
            //"Screen.PhysicalY", 
            "Config.MovingPhysicalOutsideBounds",
            "Screen.PhysicalOutsideBounds",
            //"Screen.LeftBorder",
            //"Screen.TopBorder",
            "Presenter.Ratio")]
        private void UpdateMargin()
        {
            if (Presenter == null) return;

            double x = Presenter.PhysicalToUiX(Screen.PhysicalOutsideBounds.X);
            double y = Presenter.PhysicalToUiY(Screen.PhysicalOutsideBounds.Y);

            Margin = new Thickness(x,y,0,0);
        }


        [DependsOn("Presenter.Ratio")]
        public Thickness LogoPadding => new Thickness(4 * Presenter.Ratio);

        [DependsOn("Presenter.Ratio")]
        private double Ratio => Presenter?.Ratio ?? 1;

        [DependsOn("Screen.TopBorder", "Presenter.Ratio")]
        public GridLength TopBorder => new GridLength((Screen?.TopBorder??0) * Ratio);
        [DependsOn("Screen.BottomBorder", "Presenter.Ratio")]
        public GridLength BottomBorder => new GridLength((Screen?.BottomBorder??0) * Ratio);

        [DependsOn("Screen.LeftBorder", "Presenter.Ratio")]
        public GridLength LeftBorder => new GridLength((Screen?.LeftBorder??0) * Ratio);

        [DependsOn("Screen.RightBorder", "Presenter.Ratio")]
        public GridLength RightBorder => new GridLength((Screen?.RightBorder??0) * Ratio);

        #region Unrotated

        public double UnrotatedWidth
        {
            get {  return GetProperty<double>(); }
            private set { SetProperty(value); }
        }

        public double UnrotatedHeight
        {
            get { return GetProperty<double>();  }
            private set { SetProperty(value); }
        }

        [DependsOn("Screen", "Monitor.DisplayOrientation", "Width", "Height")]
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
            get { return GetProperty<GridLength>(); }
            private set { SetProperty(value); }
        }

        public GridLength UnrotatedRightBorder
        {
            get { return GetProperty<GridLength>(); }
            private set { SetProperty(value); }
        }

        public GridLength UnrotatedBottomBorder
        {
            get { return GetProperty<GridLength>(); }
            private set { SetProperty(value); }
        }

        public GridLength UnrotatedLeftBorder
        {
            get { return GetProperty<GridLength>(); }
            private set { SetProperty(value); }
        }

        [DependsOn(nameof(LeftBorder), nameof(TopBorder), nameof(RightBorder), nameof(BottomBorder))]
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

        [DependsOn("Presenter.ScreenControlGetter", nameof(Screen))]
        private void UpdateScreenGuiControl()
        {
            ControlViewModel 
                = Presenter?.ScreenControlGetter?.GetScreenControlViewModel(Screen);

            if(ControlViewModel!=null)
            ControlViewModel.Frame = this;
        }


        public Transform ScreenOrientation
        {
            get { return GetProperty<Transform>(); }
            private set { SetProperty(value); }
        }

        [DependsOn("Monitor.DisplayOrientation", nameof(Width), nameof(Height))]
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
            get { return GetProperty<Viewbox>(); }
            private set { SetProperty(value); }
        }

        [DependsOn("Screen", "Monitor.ManufacturerCode")]
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


