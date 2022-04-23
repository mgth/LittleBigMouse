/*
  LittleBigMouse.Control.Core
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

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

using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Control.ScreenFrame
{
    using H = H<ScreenFrameViewModel>;
    public class ScreenFrameViewModel : ViewModel<Monitor>, IMvvmContextProvider, IScreenFrameViewModel
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
                .NotNull(e => e.Presenter)
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
                           (e.Model.Layout.X0 + e.Model.XMoving - e.Model.InMm.LeftBorder);
                })
                .On(e => e.Presenter.VisualRatio.X)
                .On(e => e.Model.XMoving)
                .On(e => e.Model.Layout.X0)
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
                           (e.Model.Layout.Y0 + e.Model.YMoving - e.Model.InMm.TopBorder);
                })
                .On(e => e.Presenter.VisualRatio.Y)
                .On(e => e.Model.YMoving)
                .On(e => e.Model.Layout.Y0)
                .On(e => e.Model.InMm.TopBorder)
                .Update()
            );


        public IDisplaySize Rotated => _rotated.Get();
        private readonly IProperty<IDisplaySize> _rotated = H.Property<IDisplaySize>(c => c
//            .NotNull(e => e.Model)
//            .NotNull(e => e.Presenter)
            .Set(e =>
            {
                if (e.Model == null) return null;
                if (e.Presenter == null) return null;

                //if (e.Presenter == null) return null;//e.Model.InMm;
                return e.Model.InMm.ScaleWithLocation(e.Presenter.VisualRatio);
            })
            .On(e => e.Presenter.VisualRatio)
            .On(e => e.Model.InMm)
            .Update()
        );


        public IDisplaySize Unrotated => _unrotated.Get();
        private readonly IProperty<IDisplaySize> _unrotated = H.Property<IDisplaySize>(c => c
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
                switch (e.Model.Layout.WallpaperStyle)
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
            .On(e => e.Model.Layout.WallpaperStyle)
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
                        Source = new BitmapImage(new Uri(e.Model.ActiveSource.Device.WallpaperPath)),
                        Stretch = e.WallPaperStretch,
                        HorizontalAlignment = HorizontalAlignment.Center,
                        VerticalAlignment = VerticalAlignment.Center,
                    };
                }
                catch (Exception)
                {
                    return null;
                }
            })
            .On(e => e.WallPaperStretch)
            .On(e => e.Model.ActiveSource.Device.WallpaperPath)
            .Update()
        );

        public Brush BackgroundColor => _backgroundColor.Get();
        private readonly IProperty<Brush> _backgroundColor = H.Property<Brush>(c=>c
            .Set(e => (Brush)new SolidColorBrush(
                Color.FromRgb(
                    (byte)e.Model.Layout.BackgroundColor[0],
                    (byte)e.Model.Layout.BackgroundColor[1],
                    (byte)e.Model.Layout.BackgroundColor[2])))
            .On(e => e.Model.Layout.BackgroundColor)
            .Update()
        );



        public string Logo => _logo.Get();
        private readonly IProperty<string> _logo = H.Property<string>(c => c
            .Set(e => e.GetLogo())
            .On(e => e.Model.ActiveSource.Device.Edid.ManufacturerCode)
            .On(e => e.Model.ActiveSource.Device.AttachedDevice.Parent.DeviceString)
            .Update()
        );

        private string GetLogo()
        {
            var dev = Model?.ActiveSource?.Device?.AttachedDevice?.Parent?.DeviceString;
            if(dev !=null && dev.ToLower().Contains("spacedesk")) return "icon/Pnp/Spacedesk"; 

            if (Model?.ActiveSource?.Device?.Edid?.ManufacturerCode == null) return null;

            return $"icon/Pnp/{Model.ActiveSource.Device.Edid.ManufacturerCode}";
        }



        public void ConfigureMvvmContext(IMvvmContext ctx)
        {
           ctx.AddCreator<IScreenContentViewModel>(e => e.ScreenFrameViewModel = this);
        }
    }

    
}


