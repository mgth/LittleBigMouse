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
        public ScreenFrameViewModel()
        {
            InitNotifier();
        }

        public override Type ViewType => typeof(ScreenFrameView);

        private ScreenControlViewModel _controlViewModel;
        public ScreenControlViewModel ControlViewModel
        {
            get { return _controlViewModel; }
            set { SetAndWatch(ref _controlViewModel, value); }
        }

        private MultiScreensViewModel _presenter;
         public MultiScreensViewModel Presenter
        {
            get { return _presenter; }
            set { SetAndWatch(ref _presenter, value); }
        }

        private double _width = 0;
        private double _height = 0;
        public double Width
        {
            get { return _width; }
            private set { SetProperty(ref _width, value); }
        }

        public double Height
        {
            get { return _height; }
            private set { SetProperty(ref _height, value); }
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

        private Thickness _margin = new Thickness();

        public Thickness Margin => _margin;

        [DependsOn(
            "Presenter.Size",
            //"Screen.PhysicalX",
            //"Screen.PhysicalY", 
            "Config.MovingPhysicalOutsideBounds",
            "Screen.PhysicalOutsideBounds",
            //"Screen.LeftBorder",
            //"Screen.TopBorder",
            "Presenter.Ratio")]
        private void UpdateMargin(string s)
        {
            if (s == "Config.MovingPhysicalOutsideBounds")
            {
                
            }

            if (Presenter == null) return;

            bool changed = false;
            double x = Presenter.PhysicalToUiX(Screen.PhysicalOutsideBounds.X);
            double y = Presenter.PhysicalToUiY(Screen.PhysicalOutsideBounds.Y);
            if (_margin.Left != x) { _margin.Left = x; changed = true; }
            if (_margin.Top != y) { _margin.Top = y; changed = true; }

            if (changed) RaiseProperty(nameof(Margin));
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

        private double _unrotatedWidth;
        public double UnrotatedWidth
        {
            get {  return _unrotatedWidth; }
            private set { SetProperty(ref _unrotatedWidth, value); }
        }

        private double _unrotatedHeight;
        public double UnrotatedHeight
        {
            get { return _unrotatedHeight;  }
            private set { SetProperty(ref _unrotatedHeight, value); }
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

        private GridLength _unrotatedTopBorder;
        public GridLength UnrotatedTopBorder
        {
            get { return _unrotatedTopBorder; }
            private set { SetProperty(ref _unrotatedTopBorder, value); }
        }

        private GridLength _unrotatedRightBorder;
        public GridLength UnrotatedRightBorder
        {
            get { return _unrotatedRightBorder; }
            private set { SetProperty(ref _unrotatedRightBorder, value); }
        }

        private GridLength _unrotatedBottomBorder;
        public GridLength UnrotatedBottomBorder
        {
            get { return _unrotatedBottomBorder; }
            private set { SetProperty(ref _unrotatedBottomBorder, value); }
        }

        private GridLength _unrotatedLeftBorder;
        public GridLength UnrotatedLeftBorder
        {
            get { return _unrotatedLeftBorder; }
            private set { SetProperty(ref _unrotatedLeftBorder, value); }
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
        private void UpdateScreenGuiControl(string s)
        {
            ControlViewModel 
                = Presenter?.ScreenControlGetter?.GetScreenControlViewModel(Screen);

            if(ControlViewModel!=null)
            ControlViewModel.Frame = this;
        }

        private Transform _screenOrientation;

        public Transform ScreenOrientation
        {
            get { return _screenOrientation; }
            private set { SetProperty(ref _screenOrientation, value); }
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
            get { return _logo; }
            private set { SetProperty(ref _logo, value); }
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


