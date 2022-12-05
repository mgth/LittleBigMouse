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

using System.Linq;
using System.Windows;

using HLab.Mvvm;
using HLab.Notify.PropertyChanged;

using LittleBigMouse.DisplayLayout;

namespace LittleBigMouse.Control.Plugins.Default
{
    using H = H<DefaultMonitorViewModel>;

    internal class DefaultMonitorViewModel : ViewModel<Monitor>
    {
        public DefaultMonitorViewModel() => H.Initialize(this);

        public string Inches => _inches.Get();

        readonly IProperty<string> _inches = H.Property<string>(c => c
            .Set(e => (e.Model.Diagonal / 25.4).ToString("##.#") +"\"")
            .On(e => e.Model.Diagonal)
            .Update()
        );
        public VerticalAlignment DpiVerticalAlignment => _dpiVerticalAlignment.Get();

        readonly IProperty<VerticalAlignment> _dpiVerticalAlignment = H.Property<VerticalAlignment>(c => c
            .Set(e => e.Model.Orientation == 3 ? VerticalAlignment.Bottom : VerticalAlignment.Top)
            .On(e => e.Model.Orientation)
            .Update()
        );

        public VerticalAlignment PnpNameVerticalAlignment => _pnpNameVerticalAlignment.Get();

        readonly IProperty<VerticalAlignment> _pnpNameVerticalAlignment = H.Property<VerticalAlignment>(c => c
            .Set(e => e.Model.Orientation == 2 ? VerticalAlignment.Bottom : VerticalAlignment.Top)
            .On(e => e.Model.Orientation)
            .Update()
        );
        public string InterfaceLogo => _interfaceLogo.Get();

        readonly IProperty<string> _interfaceLogo = H.Property<string>(c => c
            .Set(e => e.GetInterfaceLogo())
//            .On(e => e.Model.ActiveSource.Device.Edid.ManufacturerCode)
            .On(e => e.Model.ActiveSource.Device.AttachedDevice.Parent.DeviceString)
            .Update()
        );

        readonly string[] _brands = {"intel","amd","nvidia"};
        public string GetInterfaceLogo()
        {
            var dev = Model?.ActiveSource?.Device?.AttachedDevice?.Parent?.DeviceString;
            if(dev is string s)
            {
                var l = s.ToLower();
                foreach(var brand in _brands)
                {
                    if(l.Contains(brand)) return $"icon/pnp/{brand}";
                }
            }
            return "";
        }
        public string DeviceString => _deviceString.Get();

        readonly IProperty<string> _deviceString = H.Property<string>(c => c
            .Set(e => e.GetDeviceString())
//            .On(e => e.Model.ActiveSource.Device.Edid.ManufacturerCode)
            .On(e => e.Model.ActiveSource.Device.AttachedDevice.Parent.DeviceString)
            .Update()
        );

        string GetDeviceString()
        {
            var dev = Model?.ActiveSource?.Device?.AttachedDevice?.Parent?.DeviceString;
            if (dev is string s)
            {
                return string.Join(' ',s.Split(' ').Where(l => !_brands.Contains(l.ToLower())));
            }
            return "";
        }

    }
}
