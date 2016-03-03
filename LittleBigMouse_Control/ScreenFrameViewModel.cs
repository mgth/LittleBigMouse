using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using LittleBigMouse_Daemon.Annotations;
using NotifyChange;

namespace LittleBigMouse_Control
{
    class ScreenFrameViewModel : ScreenViewModel
    {
        public override Type ViewType => typeof(ScreenFrameView);

        public static DependencyProperty ControlViewModelProperty = DependencyProperty.Register
            (nameof(ControlViewModel), typeof(ScreenControlViewModel), typeof(ScreenFrameViewModel), WatchNotifier());

        public ScreenControlViewModel ControlViewModel
        {
            get { return (ScreenControlViewModel)GetValue(ControlViewModelProperty); }
            set { SetValue(ControlViewModelProperty, value); }
        }

        public static DependencyProperty PresenterProperty = DependencyProperty.Register
            (nameof(Presenter), typeof(MultiScreensViewModel), typeof(ScreenFrameViewModel), WatchNotifier());


        public MultiScreensViewModel Presenter
        {
            get { return (MultiScreensViewModel)GetValue(PresenterProperty); }
            set { SetValue(PresenterProperty, value); }
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



        [DependsOn("Screen.TopBorder", "Presenter.Ratio")]
        public GridLength TopBorder => new GridLength(Screen.TopBorder * Presenter.Ratio);
        [DependsOn("Screen.BottomBorder", "Presenter.Ratio")]
        public GridLength BottomBorder => new GridLength(Screen.BottomBorder * Presenter.Ratio);

        [DependsOn("Screen.LeftBorder", "Presenter.Ratio")]
        public GridLength LeftBorder => new GridLength(Screen.LeftBorder * Presenter.Ratio);

        [DependsOn("Screen.RightBorder", "Presenter.Ratio")]
        public GridLength RightBorder => new GridLength(Screen.RightBorder * Presenter.Ratio);

        #region Unrotated
        private static readonly DependencyPropertyKey UnrotatedRightBorderPropertyKey = DependencyProperty.RegisterReadOnly(nameof(UnrotatedRightBorder), typeof(GridLength), typeof(ScreenFrameViewModel), new PropertyMetadata(null));
        private static readonly DependencyPropertyKey UnrotatedLeftBorderPropertyKey = DependencyProperty.RegisterReadOnly(nameof(UnrotatedLeftBorder), typeof(GridLength), typeof(ScreenFrameViewModel), new PropertyMetadata(null));
        private static readonly DependencyPropertyKey UnrotatedTopBorderPropertyKey = DependencyProperty.RegisterReadOnly(nameof(UnrotatedTopBorder), typeof(GridLength), typeof(ScreenFrameViewModel), new PropertyMetadata(null));
        private static readonly DependencyPropertyKey UnrotatedBottomBorderPropertyKey = DependencyProperty.RegisterReadOnly(nameof(UnrotatedBottomBorder), typeof(GridLength), typeof(ScreenFrameViewModel), new PropertyMetadata(null));
        private static readonly DependencyProperty UnrotatedRightBorderProperty = UnrotatedRightBorderPropertyKey.DependencyProperty;
        private static readonly DependencyProperty UnrotatedLeftBorderProperty = UnrotatedLeftBorderPropertyKey.DependencyProperty;
        private static readonly DependencyProperty UnrotatedTopBorderProperty = UnrotatedTopBorderPropertyKey.DependencyProperty;
        private static readonly DependencyProperty UnrotatedBottomBorderProperty = UnrotatedBottomBorderPropertyKey.DependencyProperty;

        private static readonly DependencyPropertyKey UnrotatedWidthPropertyKey = DependencyProperty.RegisterReadOnly(nameof(UnrotatedWidth), typeof(double), typeof(ScreenFrameViewModel), new PropertyMetadata(null));
        private static readonly DependencyPropertyKey UnrotatedHeightPropertyKey = DependencyProperty.RegisterReadOnly(nameof(UnrotatedHeight), typeof(double), typeof(ScreenFrameViewModel), new PropertyMetadata(null));
        private static readonly DependencyProperty UnrotatedWidthProperty = UnrotatedWidthPropertyKey.DependencyProperty;
        private static readonly DependencyProperty UnrotatedHeightProperty = UnrotatedHeightPropertyKey.DependencyProperty;


        public double UnrotatedWidth
        {
            get {  return (double)GetValue(UnrotatedWidthProperty); }
            private set { SetValue(UnrotatedWidthPropertyKey, value); }
        }

        public double UnrotatedHeight
        {
            get { return (double)GetValue(UnrotatedHeightProperty);  }
            private set { SetValue(UnrotatedHeightPropertyKey, value); }
        }

        [DependsOn("Screen", "Screen.Orientation", "Width", "Height")]
        private void UpdateUnrotatedWidthHeight()
        {
            if (Screen.Orientation % 2 == 0)
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
            get { return (GridLength)GetValue(UnrotatedTopBorderProperty); }
            private set { SetValue(UnrotatedTopBorderPropertyKey, value); }
        }

        public GridLength UnrotatedRightBorder
        {
            get { return (GridLength)GetValue(UnrotatedRightBorderProperty); }
            private set { SetValue(UnrotatedRightBorderPropertyKey, value); }
        }

        public GridLength UnrotatedBottomBorder
        {
            get { return (GridLength)GetValue(UnrotatedBottomBorderProperty); }
            private set { SetValue(UnrotatedBottomBorderPropertyKey, value); }
        }

        public GridLength UnrotatedLeftBorder
        {
            get { return (GridLength)GetValue(UnrotatedLeftBorderProperty); }
            private set { SetValue(UnrotatedLeftBorderPropertyKey, value); }
        }

        [DependsOn(nameof(LeftBorder), nameof(TopBorder), nameof(RightBorder), nameof(BottomBorder))]
        private void UpdateUnrotated()
        {
            GridLength[] unrotated = { TopBorder, RightBorder, BottomBorder, LeftBorder };

            int o = Screen.Orientation;

            UnrotatedTopBorder = unrotated[o++ % 4];
            UnrotatedRightBorder = unrotated[o++ % 4];
            UnrotatedBottomBorder = unrotated[o++ % 4];
            UnrotatedLeftBorder = unrotated[o % 4];
        }
        #endregion

        [DependsOn("Presenter.GetScreenControlViewModel", nameof(Screen))]
        private void UpdateScreenGuiControl(string s)
        {
            ControlViewModel 
                = Presenter?.GetScreenControlViewModel?.Invoke(Screen);

            if(ControlViewModel!=null)
            ControlViewModel.Frame = this;
        }

        private Transform _screenOrientation;

        public Transform ScreenOrientation
        {
            get { return _screenOrientation; }
            private set { SetProperty(ref _screenOrientation, value); }
        }

        [DependsOn("Screen.Orientation", nameof(Width), nameof(Height))]
        public void UpddateScreenOrientation()
        {
                var t = new TransformGroup();
                if (Screen.Orientation > 0) t.Children.Add(new RotateTransform(90 * Screen.Orientation));

                switch (Screen.Orientation)
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

        [DependsOn("Screen.ManufacturerCode")]
        public void UpdateLogo()
        {
                switch (Screen.ManufacturerCode.ToLower())
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


