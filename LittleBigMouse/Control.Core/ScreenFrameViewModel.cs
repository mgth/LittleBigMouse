/*
  LittleBigMouse.Control.Core
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Control.Core.

    LittleBigMouse.Control.Core is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Control.Core is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using HLab.Mvvm;
using HLab.Notify;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using LittleBigMouse.ScreenConfigs;

namespace LittleBigMouse.Control.Core
{
    public class ScreenFrameViewModel : ViewModel<ScreenFrameViewModel,Screen>
    {
        public ScreenFrameViewModel()
        {
            Initialize();
        }

        public MultiScreensViewModel Presenter
        {
            get => _presenter.Get();
            set => _presenter.Set(value);
        }
        private readonly IProperty<MultiScreensViewModel> _presenter = H.Property<MultiScreensViewModel>(nameof(Presenter),c=>c);

        public TransformGroup Rotation => _rotation.Get();
        private readonly IProperty<TransformGroup> _rotation = H.Property<TransformGroup>(nameof(Rotation), c => c
        
            .On(e => e.Model.Orientation)
            .On(e => e.Height)
            .On(e => e.Width)
            .Set( e => 
            {
                var t = new TransformGroup();
                if (e.Model.Orientation > 0)
                    t.Children.Add(new RotateTransform(90 * e.Model.Orientation));

                switch (e.Model.Orientation)
                {
                    case 1:
                        t.Children.Add(new TranslateTransform(e.Width, 0));
                        break;
                    case 2:
                        t.Children.Add(new TranslateTransform(e.Width, e.Height));
                        break;
                    case 3:
                        t.Children.Add(new TranslateTransform(0, e.Height));
                        break;
                }

                return t;
            }
                ));



        public double Ratio
        {
            get => _ratio.Get();
            set => _ratio.Set(value);
        }
        private readonly IProperty<double> _ratio = H.Property<double>(nameof(Ratio), c=> c
            .Set(e => 1.0));


        public Thickness LogoPadding => _logoPadding.Get();
        private readonly IProperty<Thickness> _logoPadding = H.Property<Thickness>(nameof(LogoPadding), c => c
                .On(e => e.Ratio)
                .Set(e => new Thickness(4 * e.Ratio))
        );


        private GridLength GetLength(double l) => new GridLength(GetSize(l));
        private double GetSize(double l) => l * Ratio;

        public Thickness Margin => _margin.Get();
        private readonly IProperty<Thickness> _margin 
            = H.Property<Thickness>(nameof(Margin), c => c
                .On(e => e.Ratio)
                .On(e => e.Model.XMoving)
                .On(e => e.Model.YMoving)
                .On(e => e.Model.Config.PhysicalOutsideBounds.Left)
                .On(e => e.Model.Config.PhysicalOutsideBounds.Top)
                .On(e => e.Model.Config.PhysicalOutsideBounds.Height)
                .On(e => e.Model.Config.PhysicalOutsideBounds.Width)
                .On(e => e.Model.InMm.LeftBorder)
                .On(e => e.Model.InMm.TopBorder)
                .Set(e => new Thickness(
                    e.GetSize(e.Model.XMoving - e.Model.InMm.LeftBorder - e.Model.Config.PhysicalOutsideBounds.Left - e.Model.Config.PhysicalOutsideBounds.Width/2),
                    e.GetSize(e.Model.YMoving - e.Model.InMm.TopBorder - e.Model.Config.PhysicalOutsideBounds.Top - e.Model.Config.PhysicalOutsideBounds.Height/2),0,0
        ))
            );

        // Height
        public double Height => _height.Get();
        private readonly IProperty<double> _height = H.Property<double>(nameof(Height), c => c
                .On(e => e.Ratio)
                .On(e => e.Model.InMm.OutsideHeight)
                .Set( (e => e.GetSize(e.Model.InMm.OutsideHeight)))
        );

        // Width
        public double Width => _width.Get();
        private readonly IProperty<double> _width = H.Property<double>(nameof(Width), c => c
            .On(e => e.Ratio)
            .On(e => e.Model.InMm.OutsideWidth)
            .Set( (e => e.GetSize(e.Model.InMm.OutsideWidth)))
        );

        // TopBorder
        public GridLength TopBorder => _topBorder.Get();
        private readonly IProperty<GridLength> _topBorder = H.Property<GridLength>(c=>c
            .On(e => e.Ratio)
            .On(e => e.Model.InMm.TopBorder)
            .Set(e => e.GetLength(e.Model.InMm.TopBorder))
        );

        // RightBorder
        public GridLength RightBorder => _rightBorder.Get();
        private readonly IProperty<GridLength> _rightBorder = H.Property<GridLength>(nameof(RightBorder), c => c
            .On(e => e.Ratio)
            .On(e => e.Model.InMm.RightBorder)
            .Set(e => e.GetLength(e.Model.InMm.RightBorder))
        );

        // BottomBorder
        public GridLength BottomBorder => _bottomBorder.Get();
        private readonly IProperty<GridLength> _bottomBorder = H.Property<GridLength>(nameof(BottomBorder), c => c
            .On(e => e.Ratio)
            .On(e => e.Model.InMm.BottomBorder)
            .Set(e => e.GetLength(e.Model.InMm.BottomBorder))        
        );

        // LeftBorder
        public GridLength LeftBorder => _leftBorder.Get();
        private readonly IProperty<GridLength> _leftBorder = H.Property<GridLength>(nameof(LeftBorder), c => c
            .On(e => e.Ratio)
            .On(e => e.Model.InMm.LeftBorder)   
            .Set(e => e.GetLength(e.Model.InMm.LeftBorder))
        );


        // UnrotatedHeight
        public double UnrotatedHeight => _unrotatedHeight.Get();
        private readonly IProperty<double> _unrotatedHeight = H.Property<double>(nameof(UnrotatedHeight), c => c
            .On(e => e.Ratio)
            .On(e => e.Model.InMmUnrotated.OutsideHeight)
            .Set(e => e.GetSize(e.Model.InMmUnrotated.OutsideHeight))
        );

        // UnrotatedWidth
        public double UnrotatedWidth => _unrotatedWidth.Get();
        private readonly IProperty<double> _unrotatedWidth = H.Property<double>(nameof(UnrotatedWidth), c => c
            .On(e => e.Ratio)
            .On(e => e.Model.InMmUnrotated.OutsideWidth)
            .Set(e => e.GetSize(e.Model.InMmUnrotated.OutsideWidth))
            );


        // UnrotatedTopBorder
        public GridLength UnrotatedTopBorder => _unrotatedTopBorder.Get();
        private readonly IProperty<GridLength> _unrotatedTopBorder = H.Property<GridLength>(nameof(UnrotatedTopBorder),c=>c
            .On(e => e.Ratio)
            .On(e => e.Model.InMmUnrotated.TopBorder)
            .Set(e => e.GetLength(e.Model.InMmUnrotated.TopBorder))
            );

        // UnrotatedRightBorder
        public GridLength UnrotatedRightBorder => _unrotatedRightBorder.Get();
        private readonly IProperty<GridLength> _unrotatedRightBorder = H.Property<GridLength>(nameof(UnrotatedRightBorder),c=>c
            .On(e => e.Ratio)
            .On(e => e.Model.InMmUnrotated.RightBorder)
            .Set(e => e.GetLength(e.Model.InMmUnrotated.RightBorder))
        );

        // UnrotatedBottomBorder
        public GridLength UnrotatedBottomBorder => _unrotatedBottomBorder.Get();
        private readonly IProperty<GridLength> _unrotatedBottomBorder = H.Property<GridLength>(nameof(UnrotatedBottomBorder),c=>c
            .On(e => e.Ratio)
            .On(e => e.Model.InMmUnrotated.BottomBorder)
            .Set(e => e.GetLength(e.Model.InMmUnrotated.BottomBorder))           
        );

        // UnrotatedLeftBorder
        public GridLength UnrotatedLeftBorder => _unrotatedLeftBorder.Get();
        private readonly IProperty<GridLength> _unrotatedLeftBorder = H.Property<GridLength>(nameof(UnrotatedLeftBorder),c=>c
            .On(e => e.Ratio)
            .On(e => e.Model.InMmUnrotated.LeftBorder)
            .Set(e => e.GetLength(e.Model.InMmUnrotated.LeftBorder))                   
        );



        public Stretch WallPaperStretch => _wallPaperStretch.Get();
        private readonly IProperty<Stretch> _wallPaperStretch = H.Property<Stretch>(nameof(WallPaperStretch), c => c
            .On(e => e.Model.Config.WallpaperStyle)
            .Set(e =>
            {
                switch (e.Model.Config.WallpaperStyle)
                {
                    case 0:
                        return Stretch.None;
                    case 2:
                        return Stretch.Fill;
                    case 6:
                        return Stretch.Uniform;
                    case 10:
                        return Stretch.UniformToFill;
                    case 22: // stretched across all screens
                    default:
                        return Stretch.None;
                }
            })
        
        );


        public Image WallPaper => _wallPaper.Get();
        private readonly IProperty<Image> _wallPaper = H.Property<Image>(nameof(WallPaper),c => c
            .On(e => e.WallPaperStretch)
            .On(e => e.Model.Config.WallPaperPath)
            .Set(e =>
            {
                try
                {
                    return new Image
                    {
                        Source = new BitmapImage(new Uri(e.Model.Config.WallPaperPath)),
                        Stretch = e.WallPaperStretch,
                        HorizontalAlignment = HorizontalAlignment.Center
                    };
                }
                catch (Exception)
                {
                    return null;
                }
            })
        );

        public Brush BackgroundColor => _backgroundColor.Get();
        private readonly IProperty<Brush> _backgroundColor = H.Property<Brush>(nameof(BackgroundColor),c=>c
            .On(e => e.Model.Config.BackGroundColor)
            .Set(e => new SolidColorBrush(
                Color.FromRgb(
                    (byte)e.Model.Config.BackGroundColor[0],
                    (byte)e.Model.Config.BackGroundColor[1],
                    (byte)e.Model.Config.BackGroundColor[2])))
        );



        public Viewbox Logo => _logo.Get();
        private readonly IProperty<Viewbox> _logo = H.Property<Viewbox>(nameof(Logo), c => c
            .Set(e => e.GetLogo())
        
        );

        public Viewbox GetLogo()
        {
            var dev = Model?.Monitor?.AttachedDevice?.Parent?.DeviceString;
            if(dev !=null && dev.ToLower().Contains("spacedesk")) return (Viewbox)Application.Current.FindResource("LogoSpacedesk"); 

            if (Model?.Monitor?.Edid?.ManufacturerCode == null) return null;

                switch (Model.Monitor.Edid.ManufacturerCode.ToLower())
                {

                // https://github.com/OCSInventory-NG/WindowsAgent/blob/master/SysInfo/ISA_PNPID.cpp

                case "sam":
                case "skt":
                case "sse":
                case "stn":
                case "kyk":
                case "sem":
                    return (Viewbox)Application.Current.FindResource("LogoSam"); 
                case "del":
                case "dll":
                    return (Viewbox)Application.Current.FindResource("LogoDel"); 
                case "che":
                case "ali":
                case "acr":
                case "api":
                    return (Viewbox)Application.Current.FindResource("LogoAcer"); 
                case "atk":
                case "aci":
                case "asu":
                    return (Viewbox)Application.Current.FindResource("LogoAsus"); 
                case "eiz":
                case "egd":
                case "enc":
                    return (Viewbox)Application.Current.FindResource("LogoEizo"); 
                case "ben":
                case "bnq":
                    return (Viewbox)Application.Current.FindResource("LogoBenq");
                case "nec":
                case "nct":
                case "nmv":
                    return (Viewbox)Application.Current.FindResource("LogoNec"); 
                case "hpq":
                case "hpd":
                case "hpc":
                    return (Viewbox)Application.Current.FindResource("LogoHp");
                case "lg":
                case "lgs":
                case "gsm"://GoldStar 
                    return (Viewbox)Application.Current.FindResource("LogoLg"); 
                case "apl":
                case "app":
                    return (Viewbox)Application.Current.FindResource("LogoApple");
                case "fdt":
                case "fuj":
                case "fmi":
                case "fml":
                case "fpe":
                case "fus":
                case "fjs":
                case "fjc":
                case "ftl":
                    return (Viewbox)Application.Current.FindResource("LogoFujitsu");
                case "ibm":
                case "cdt":
                    return (Viewbox)Application.Current.FindResource("LogoIbm");
                case "mat":
                case "mdo":
                case "plf":
                case "mei":
                    return (Viewbox)Application.Current.FindResource("LogoPanasonic");
                case "sny":
                case "son":
                case "ser":
                    return (Viewbox)Application.Current.FindResource("LogoSony");
                case "tai":
                case "tsb":
                case "tos":
                case "tgc":
                case "lcd":
                case "pcs":
                case "tli":
                    return (Viewbox)Application.Current.FindResource("LogoToshiba");
                case "aoc":
                    return (Viewbox)Application.Current.FindResource("LogoAoc");
                case "ivm":
                    return (Viewbox)Application.Current.FindResource("LogoIiyama");
                case "len":
                case "lnv":
                case "lin":
                    return (Viewbox)Application.Current.FindResource("LogoLenovo");
                case "pca":
                case "phs":
                case "phl":
                case "phe":
                case "psc":
                    return (Viewbox)Application.Current.FindResource("LogoPhilips");
                case "hei":
                    return (Viewbox)Application.Current.FindResource("LogoYundai");
                case "cpq":
                    return (Viewbox)Application.Current.FindResource("LogoCompaq");
                case "hit":
                case "hcp":
                case "hce":
                case "hec":
                case "hic":
                case "htc":
                case "mxl":
                case "hel":
                    return (Viewbox)Application.Current.FindResource("LogoHitachi");
                case "hyo":
                    return (Viewbox)Application.Current.FindResource("LogoQnix");
                case "nts":
                    return (Viewbox)Application.Current.FindResource("LogoIolair");
                case "otm":
                    return (Viewbox)Application.Current.FindResource("LogoOptoma");
                case "vsc":
                    return (Viewbox)Application.Current.FindResource("LogoViewsonic");
                case "msg":
                    return (Viewbox)Application.Current.FindResource("LogoMsi");
                default:
                    return (Viewbox)Application.Current.FindResource("LogoLbm");
            }
            }

    }

    
}


