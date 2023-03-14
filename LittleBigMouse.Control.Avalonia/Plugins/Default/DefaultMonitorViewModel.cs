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
using Avalonia.Layout;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Plugins.Default;

internal class DefaultMonitorViewModel : ViewModel<Monitor>
{
    public DefaultMonitorViewModel()
    {
        _inches = this.WhenAnyValue(
                e => e.Model.Diagonal,
                selector: d => (d / 25.4).ToString("##.#") + "\"")
            .ToProperty(this, _ => _.Inches);

        _dpiVerticalAlignment = this.WhenAnyValue(
            e => e.Model.Orientation,
            selector: o => o == 3 ? VerticalAlignment.Bottom : VerticalAlignment.Top
        ).ToProperty(this, _ => _.DpiVerticalAlignment);

        _pnpNameVerticalAlignment = this.WhenAnyValue(
            e => e.Model.Orientation,
            selector: o => o == 2 ? VerticalAlignment.Bottom : VerticalAlignment.Top
        ).ToProperty(this, _ => _.PnpNameVerticalAlignment);

        _interfaceLogo = this.WhenAnyValue(
            _ => _.Model.ActiveSource.Device.AttachedDevice.Parent.DeviceString,
            GetInterfaceLogo
        ).ToProperty(this, _ => _.InterfaceLogo);

        _deviceString = this.WhenAnyValue(
            _ => _.Model.ActiveSource.Device.AttachedDevice.Parent.DeviceString,
            GetDeviceString
        ).ToProperty(this, _ => _.InterfaceLogo);
    }

    public string Inches => _inches.Value;
    readonly ObservableAsPropertyHelper<string> _inches;

    public VerticalAlignment DpiVerticalAlignment => _dpiVerticalAlignment.Value;
    readonly ObservableAsPropertyHelper<VerticalAlignment> _dpiVerticalAlignment;

    public VerticalAlignment PnpNameVerticalAlignment => _pnpNameVerticalAlignment.Value;
    readonly ObservableAsPropertyHelper<VerticalAlignment> _pnpNameVerticalAlignment;

    public string InterfaceLogo => _interfaceLogo.Value;
    readonly ObservableAsPropertyHelper<string> _interfaceLogo;

    static readonly string[] Brands = { "intel", "amd", "nvidia" };
    public static string GetInterfaceLogo(string? deviceString)
    {
        if (deviceString is not { }) return "";

        var l = deviceString.ToLower();
        foreach (var brand in Brands)
        {
            if (l.Contains(brand)) return $"icon/pnp/{brand}";
        }
        return "";
    }
        
    public string DeviceString => _deviceString.Value;
    readonly ObservableAsPropertyHelper<string> _deviceString;

    static string GetDeviceString(string? deviceString)
    {
        return deviceString != null ? string.Join(' ', deviceString.Split(' ').Where(l => !Brands.Contains(l.ToLower()))) : "";
    }

}