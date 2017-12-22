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
using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using Hlab.Mvvm;
using Hlab.Notify;
using LittleBigMouse.ScreenConfigs;

namespace LittleBigMouse.Control.Core
{
    public class ScreenFrameViewModel : IViewModel<Screen>
    {
        public ScreenFrameViewModel()
        {
            this.Subscribe();
        }

        public Screen Model => this.Get<Screen>();

        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }

        public MultiScreensViewModel Presenter
        {
            get => this.Get<MultiScreensViewModel>();
            set => this.Set(value);
        }

        [TriggedOn(nameof(Model), "Orientation")]
        [TriggedOn(nameof(Height))]
        [TriggedOn(nameof(Width))]
        public TransformGroup Rotation => this.Get(() =>
        {
            var t = new TransformGroup();
            if (Model.Orientation > 0)
                t.Children.Add(new RotateTransform(90 * Model.Orientation));

            switch (Model.Orientation)
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

            return t;
        });



        public double Ratio
        {
            get => this.Get(() => 1.0);
            set => this.Set(value);
        }

        [TriggedOn(nameof(Ratio))]
        public Thickness LogoPadding => this.Get(() => new Thickness(4 * Ratio));





        private GridLength GetLength(double l) => new GridLength(GetSize(l));
        private double GetSize(double l) => l * Ratio;

        [TriggedOn(nameof(Ratio))]
        [TriggedOn("Model.XMoving")]
        [TriggedOn("Model.YMoving")]
        [TriggedOn("Model.Config.PhysicalOutsideBounds.Left")]
        [TriggedOn("Model.Config.PhysicalOutsideBounds.Top")]
        [TriggedOn("Model.Config.PhysicalOutsideBounds.Height")]
        [TriggedOn("Model.Config.PhysicalOutsideBounds.Width")]
        [TriggedOn("Model.InMm.LeftBorder")]
        [TriggedOn("Model.InMm.TopBorder")]
        public Thickness Margin => this.Get(() => new Thickness(
            GetSize(Model.XMoving - Model.InMm.LeftBorder - Model.Config.PhysicalOutsideBounds.Left - Model.Config.PhysicalOutsideBounds.Width/2),
            GetSize(Model.YMoving - Model.InMm.TopBorder - Model.Config.PhysicalOutsideBounds.Top - Model.Config.PhysicalOutsideBounds.Height/2),0,0
        ));


        [TriggedOn(nameof(Ratio))]
        [TriggedOn("Model.InMm.OutsideHeight")]
        public double Height => this.Get(() => GetSize(Model.InMm.OutsideHeight));
        [TriggedOn(nameof(Ratio))]
        [TriggedOn("Model.InMm.OutsideWidth")]
        public double Width => this.Get(() => GetSize(Model.InMm.OutsideWidth));
        [TriggedOn(nameof(Ratio))]
        [TriggedOn("Model.InMm.TopBorder")]
        public GridLength TopBorder => this.Get(() => GetLength(Model.InMm.TopBorder));
        [TriggedOn(nameof(Ratio))]
        [TriggedOn("Model.InMm.RightBorder")]
        public GridLength RightBorder => this.Get(() => GetLength(Model.InMm.RightBorder));
        [TriggedOn(nameof(Ratio))]
        [TriggedOn("Model.InMm.BottomBorder")]
        public GridLength BottomBorder => this.Get(() => GetLength(Model.InMm.BottomBorder));
        [TriggedOn(nameof(Ratio))]
        [TriggedOn("Model.InMm.LeftBorder")]
        public GridLength LeftBorder => this.Get(() => GetLength(Model.InMm.LeftBorder));



        [TriggedOn(nameof(Ratio))]
        [TriggedOn(nameof(Model),"InMmUnrotated","OutsideHeight")]
        public double UnrotatedHeight => this.Get(()=> GetSize(Model.InMmUnrotated.OutsideHeight));
        [TriggedOn(nameof(Ratio))]
        [TriggedOn(nameof(Model), "InMmUnrotated","OutsideWidth")]
        public double UnrotatedWidth => this.Get(()=> GetSize(Model.InMmUnrotated.OutsideWidth));
        [TriggedOn(nameof(Ratio))]
        [TriggedOn(nameof(Model), "InMmUnrotated","TopBorder")]
        public GridLength UnrotatedTopBorder => this.Get(() => GetLength(Model.InMmUnrotated.TopBorder));
        [TriggedOn(nameof(Ratio))]
        [TriggedOn(nameof(Model), "InMmUnrotated","RightBorder")]
        public GridLength UnrotatedRightBorder => this.Get(() => GetLength(Model.InMmUnrotated.RightBorder));
        [TriggedOn(nameof(Ratio))]
        [TriggedOn(nameof(Model), "InMmUnrotated","BottomBorder")]
        public GridLength UnrotatedBottomBorder => this.Get(() => GetLength(Model.InMmUnrotated.BottomBorder));
        [TriggedOn(nameof(Ratio))]
        [TriggedOn(nameof(Model), "InMmUnrotated","LeftBorder")]
        public GridLength UnrotatedLeftBorder => this.Get(() => GetLength(Model.InMmUnrotated.LeftBorder));
 

         public Viewbox Logo
        {
            get => this.Get<Viewbox>(); private set => this.Set(value);
        }

        [TriggedOn(nameof(Model),"Monitor","Edid","ManufacturerCode")]
        public void UpdateLogo()
        {
            if (Model?.Monitor?.Edid?.ManufacturerCode == null) return;

                switch (Model.Monitor.Edid.ManufacturerCode.ToLower())
                {

                // https://github.com/OCSInventory-NG/WindowsAgent/blob/master/SysInfo/ISA_PNPID.cpp

                case "sam":
                case "skt":
                case "sse":
                case "stn":
                case "kyk":
                case "sem":
                    Logo = (Viewbox)Application.Current.FindResource("LogoSam"); return;
                case "del":
                case "dll":
                    Logo = (Viewbox)Application.Current.FindResource("LogoDel"); return;
                case "che":
                case "ali":
                case "acr":
                case "api":
                    Logo = (Viewbox)Application.Current.FindResource("LogoAcer"); return;
                case "atk":
                case "aci":
                case "asu":
                    Logo = (Viewbox)Application.Current.FindResource("LogoAsus"); return;
                case "eiz":
                case "egd":
                case "enc":
                    Logo = (Viewbox)Application.Current.FindResource("LogoEizo"); return;
                case "ben":
                case "bnq":
                    Logo = (Viewbox)Application.Current.FindResource("LogoBenq"); return;
                case "nec":
                case "nct":
                case "nmv":
                    Logo = (Viewbox)Application.Current.FindResource("LogoNec"); return;
                case "hpq":
                case "hpd":
                case "hpc":
                    Logo = (Viewbox)Application.Current.FindResource("LogoHp"); return;
                case "lg":
                case "lgs":
                    Logo = (Viewbox)Application.Current.FindResource("LogoLg"); return;
                case "apl":
                case "app":
                    Logo = (Viewbox)Application.Current.FindResource("LogoApple"); return;
                case "fdt":
                case "fuj":
                case "fmi":
                case "fml":
                case "fpe":
                case "fus":
                case "fjs":
                case "fjc":
                case "ftl":
                   Logo = (Viewbox)Application.Current.FindResource("LogoFujitsu"); return;
                case "ibm":
                case "cdt":
                    Logo = (Viewbox)Application.Current.FindResource("LogoIbm"); return;
                case "mat":
                case "mdo":
                case "plf":
                case "mei":
                    Logo = (Viewbox)Application.Current.FindResource("LogoPanasonic"); return;
                case "sny":
                case "son":
                case "ser":
                    Logo = (Viewbox)Application.Current.FindResource("LogoSony"); return;
                case "tai":
                case "tsb":
                case "tos":
                case "tgc":
                case "lcd":
                case "pcs":
                case "tli":
                    Logo = (Viewbox)Application.Current.FindResource("LogoToshiba"); return;
                case "aoc":
                    Logo = (Viewbox)Application.Current.FindResource("LogoAoc"); return;
                case "ivm":
                    Logo = (Viewbox)Application.Current.FindResource("LogoIiyama"); return;
                case "len":
                case "lnv":
                case "lin":
                    Logo = (Viewbox)Application.Current.FindResource("LogoLenovo"); return;
                case "pca":
                case "phs":
                case "phl":
                case "phe":
                case "psc":
                    Logo = (Viewbox)Application.Current.FindResource("LogoPhilips"); return;
                case "hei":
                    Logo = (Viewbox)Application.Current.FindResource("LogoYundai"); return;
                case "cpq":
                    Logo = (Viewbox)Application.Current.FindResource("LogoCompaq"); return;
                case "hit":
                case "hcp":
                case "hce":
                case "hec":
                case "hic":
                case "htc":
                case "mxl":
                case "hel":
                    Logo = (Viewbox)Application.Current.FindResource("LogoHitachi"); return;
                case "hyo":
                    Logo = (Viewbox)Application.Current.FindResource("LogoQnix"); return;
                case "nts":
                    Logo = (Viewbox)Application.Current.FindResource("LogoIolair"); return;
                case "otm":
                    Logo = (Viewbox)Application.Current.FindResource("LogoOptoma"); return;
                case "vsc":
                    Logo = (Viewbox)Application.Current.FindResource("LogoViewsonic"); return;
                case "msg":
                    Logo = (Viewbox)Application.Current.FindResource("LogoMsi"); return;
                default:
                    Logo = (Viewbox)Application.Current.FindResource("LogoLbm"); return;
            }
            }

    }

    
}


