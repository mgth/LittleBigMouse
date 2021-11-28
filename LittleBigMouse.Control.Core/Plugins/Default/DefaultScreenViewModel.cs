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

using System.Windows;
using HLab.Mvvm;
using HLab.Notify.PropertyChanged;
using LittleBigMouse.ScreenConfig;

namespace LittleBigMouse.Control.Plugins.Default
{
    using H = H<DefaultScreenViewModel>;

    class DefaultScreenViewModel : ViewModel<Screen>
    {
        public DefaultScreenViewModel() => H.Initialize(this);

        public string Inches => _inches.Get();
        private readonly IProperty<string> _inches = H.Property<string>(c => c
            .Set(e => (e.Model.Diagonal / 25.4).ToString("##.#") +"\"")
            .On(e => e.Model.Diagonal)
            .Update()
        );
        public VerticalAlignment DpiVerticalAlignment => _dpiVerticalAlignment.Get();
        private readonly IProperty<VerticalAlignment> _dpiVerticalAlignment = H.Property<VerticalAlignment>(c => c
            .Set(e => e.Model.Orientation == 3 ? VerticalAlignment.Bottom : VerticalAlignment.Top)
            .On(e => e.Model.Orientation)
            .Update()
        );

        public VerticalAlignment PnpNameVerticalAlignment => _pnpNameVerticalAlignment.Get();
        private readonly IProperty<VerticalAlignment> _pnpNameVerticalAlignment = H.Property<VerticalAlignment>(c => c
            .Set(e => e.Model.Orientation == 2 ? VerticalAlignment.Bottom : VerticalAlignment.Top)
            .On(e => e.Model.Orientation)
            .Update()
        );

    }
}
