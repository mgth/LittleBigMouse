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

//#define uglyfix

using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Notify.PropertyChanged;
using LittleBigMouse.Plugins;
using LittleBigMouse.ScreenConfig;
using LittleBigMouse.ScreenConfig.Dimensions;

namespace LittleBigMouse.Control.ScreenFrame
{
    using H = H<ScreenFrameViewModel>;
    public class ScreenFrameViewModel : ViewModel<Screen>, IMvvmContextProvider, IScreenFrameViewModel
    {
        public ScreenFrameViewModel()
        {
            H.Initialize(this);
        }

        public IMultiScreensViewModel Presenter
        {
            get => _presenter.Get();
            set => _presenter.Set(value);
        }
        private readonly IProperty<IMultiScreensViewModel> _presenter = H.Property<IMultiScreensViewModel>();



        public TransformGroup Rotation => _rotation.Get();
        private readonly IProperty<TransformGroup> _rotation = H.Property<TransformGroup>(c => c
        
            .Set( e => 
            {
                if (e.Model.Orientation > 0)
                {
                    var t = new TransformGroup();
                    t.Children.Add(new RotateTransform(90 * e.Model.Orientation));
                    switch (e.Model.Orientation)
                    {
                        case 1:
                            t.Children.Add(new TranslateTransform(e.Rotated.Width, 0));
                            break;
                        case 2:
                            t.Children.Add(new TranslateTransform(e.Rotated.Width, e.Rotated.Height));
                            break;
                        case 3:
                            t.Children.Add(new TranslateTransform(0, e.Rotated.Height));
                            break;
                    }
                    return t;
                }

                return null;
            }
                )            
            .On(e => e.Model.Orientation)
            .On(e => e.Rotated.Height)
            .On(e => e.Rotated.Width)
            .Update()
        );





        public Thickness LogoPadding => _logoPadding.Get();
        private readonly IProperty<Thickness> _logoPadding = H.Property<Thickness>(c => c
                .Set(e => new Thickness(4 * e.Presenter.VisualRatio.X,4*e.Presenter.VisualRatio.Y,4 * e.Presenter.VisualRatio.X,4*e.Presenter.VisualRatio.Y))
                .On(e => e.Presenter.VisualRatio.X)
                .On(e => e.Presenter.VisualRatio.Y)
                .Update()
        );


        public Thickness Margin => _margin.Get();

        private readonly IProperty<Thickness> _margin = H.Property<Thickness>(c => c
            .Set(e => new Thickness(e.Left,e.Top,0,0))
            .On(e => e.Left)
            .On(e => e.Top)
            .Update()
        );


        public double Left => _left.Get();
        private readonly IProperty<double> _left
            = H.Property<double>(c => c
                .Set(e =>
                {
                    if (e.Presenter == null) return 0.0;

                    return e.Presenter.VisualRatio.X *
                           (e.Model.Config.X0 + e.Model.XMoving - e.Model.InMm.LeftBorder);
                })
                .On(e => e.Presenter.VisualRatio.X)
                .On(e => e.Model.XMoving)
                .On(e => e.Model.Config.X0)
                .On(e => e.Model.InMm.LeftBorder)
                .Update()
            );

        public double Top => _top.Get();
        private readonly IProperty<double> _top 
            = H.Property<double>(c => c
                .Set(e =>
                {
                    if (e.Presenter == null) return 0.0;

                    return e.Presenter.VisualRatio.Y *
                           (e.Model.Config.Y0 + e.Model.YMoving - e.Model.InMm.TopBorder);
                })
                .On(e => e.Presenter.VisualRatio.Y)
                .On(e => e.Model.YMoving)
                .On(e => e.Model.Config.Y0)
                .On(e => e.Model.InMm.TopBorder)
                .Update()
            );


        public IScreenSize Rotated => _rotated.Get();
        private readonly IProperty<IScreenSize> _rotated = H.Property<IScreenSize>(c => c
            .NotNull(e => e.Model)
            .NotNull(e => e.Presenter)
            .Set(e =>
            {
                //if (e.Presenter == null) return null;//e.Model.InMm;
                return e.Model.InMm.ScaleWithLocation(e.Presenter.VisualRatio);
            })
            .On(e => e.Presenter.VisualRatio)
            .On(e => e.Model.InMm)
            .Update()
        );


        public IScreenSize Unrotated => _unrotated.Get();
        private readonly IProperty<IScreenSize> _unrotated = H.Property<IScreenSize>(c => c
            .Set(e =>
            {
                if (e.Presenter == null) return null;//e.Model.InMmU;
                return e.Model.InMmU.ScaleWithLocation(e.Presenter.VisualRatio);
            })
            .On(e => e.Presenter.VisualRatio)
            .On(e => e.Model.InMmU)
//            .On(e => e.Model.Orientation)
            .Update()
        );





        public Stretch WallPaperStretch => _wallPaperStretch.Get();
        private readonly IProperty<Stretch> _wallPaperStretch = H.Property<Stretch>(c => c
            .NotNull(e => e.Model)
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
            .On(e => e.Model.Config.WallpaperStyle)
            .Update()
        );


        public Image WallPaper => _wallPaper.Get();
        private readonly IProperty<Image> _wallPaper = H.Property<Image>(c => c
            .NotNull(e => e.Model)
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
            .On(e => e.WallPaperStretch)
            .On(e => e.Model.Config.WallPaperPath)
            .Update()
        );

        public Brush BackgroundColor => _backgroundColor.Get();
        private readonly IProperty<Brush> _backgroundColor = H.Property<Brush>(c=>c
            .Set(e => (Brush)new SolidColorBrush(
                Color.FromRgb(
                    (byte)e.Model.Config.BackgroundColor[0],
                    (byte)e.Model.Config.BackgroundColor[1],
                    (byte)e.Model.Config.BackgroundColor[2])))
            .On(e => e.Model.Config.BackgroundColor)
            .Update()
        );



        public Viewbox Logo => _logo.Get();
        private readonly IProperty<Viewbox> _logo = H.Property<Viewbox>(c => c
            .Set(e => e.GetLogo())
            .On(e => e.Model.Monitor.Edid.ManufacturerCode)
            .On(e => e.Model.Monitor.AttachedDevice.Parent.DeviceString)
            .Update()
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
                case "gbt":
                    return (Viewbox)Application.Current.FindResource("LogoAorus");
                case "ins":
                    return (Viewbox)Application.Current.FindResource("LogoInsignia");
                case "auo":
                    return (Viewbox)Application.Current.FindResource("LogoAUO");
                case "wac":
                    return (Viewbox)Application.Current.FindResource("LogoWacom");
                    
                default:
                    return (Viewbox)Application.Current.FindResource("LogoLbm");
            }
            }

        public void ConfigureMvvmContext(IMvvmContext ctx)
        {
           ctx.AddCreator<IScreenContentViewModel>(e => e.ScreenFrameViewModel = this);
        }
    }

    
}


