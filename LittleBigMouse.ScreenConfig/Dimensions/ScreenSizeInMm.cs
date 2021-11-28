/*
  LittleBigMouse.Screen.Config
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Screen.Config.

    LittleBigMouse.Screen.Config is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Screen.Config is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Text.Json.Serialization;
using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.Monitors;

namespace LittleBigMouse.ScreenConfig.Dimensions
{
    using H = H<ScreenSizeInMm>;

    /// <summary>
    /// Actual real monitor size 
    /// </summary>
    public class ScreenSizeInMm : ScreenSize
    {
        [JsonIgnore]
        public ScreenModel ScreenModel { get; }
        public ScreenSizeInMm(ScreenModel screenModel):base(null)
        {
            ScreenModel = screenModel;
            H.Initialize(this);
        }

        public bool Saved
        {
            get => _saved.Get();
            set => _saved.Set(value);
        }
        private readonly IProperty<bool> _saved = H.Property<bool>();

        public bool FixedAspectRatio
        {
            get => _fixedAspectRatio.Get();
            set => _fixedAspectRatio.Set(value);
        }
        private readonly  IProperty<bool> _fixedAspectRatio = H.Property<bool>(c=>c
                .Set(e => true)
            );//PropertyBuilder.DefaultValue(true);

        public override double Width
        {
            get => _width.Get();

            set => _width.Set(Math.Max(value, 0), (oldValue, newValue) =>
            {
                if (FixedAspectRatio)
                {
                    var ratio = newValue / oldValue;
                    FixedAspectRatio = false;
                    Height *= ratio;
                    FixedAspectRatio = true;
                }

                Saved = false;
            });
        }
        private readonly IProperty<double> _width = H.Property<double>();

        public override double Height
        {
            get => _height.Get();
            set
            {
                _height.Set(Math.Max(value, 0), (oldValue, newValue) =>
                {
                    if (FixedAspectRatio)
                    {
                        var ratio = newValue / oldValue;
                        FixedAspectRatio = false;
                        Width *= ratio;
                        FixedAspectRatio = true;
                    }

                    Saved = false;
                } );
            }
        }
        private readonly IProperty<double> _height = H.Property<double>();

        public override double X
        {
            get => _x.Get();
            set => _x.Set(value);
        }
        private readonly IProperty<double> _x = H.Property<double>();
        
        public override double Y
        {
            get => _y.Get();
            set => _y.Set(value);
        }
        private readonly IProperty<double> _y = H.Property<double>();


        public override double TopBorder
        {
            get => _topBorder.Get();
            set => _topBorder.Set(Math.Max(value,0.0));
        }
        private readonly IProperty<double> _topBorder = H.Property<double>(c=>c
            .Set(e=>20.0)
        );

        public override double BottomBorder
        {
            get => _bottomBorder.Get();
            set => _bottomBorder.Set(Math.Max(value, 0.0));
        }
        private readonly IProperty<double> _bottomBorder = H.Property<double>(c => c
            .Set(e => 20.0)
        );

        public override double LeftBorder
        {
            get => _leftBorder.Get();
            set => _leftBorder.Set(Math.Max(value, 0.0));
        }
        private readonly IProperty<double> _leftBorder = H.Property<double>(c=>c
            .Set(e=>20.0)
        );

        public override double RightBorder
        {
            get => _rightBorder.Get();
            set => _rightBorder.Set(Math.Max(value, 0.0));
        }
        private readonly IProperty<double> _rightBorder = H.Property<double>(c => c
            .Set(e => 20.0)
        );
    }
}