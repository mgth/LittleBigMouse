/*
  LittleBigMouse.Screen.Config
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

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
using System.Runtime.CompilerServices;
using Avalonia;
using Microsoft.Win32;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public class ScreenSizeInPixels : DisplaySize
{
    public MonitorSource MonitorSource { get; }

    public ScreenSizeInPixels(MonitorSource source) : base(null)
    {
        MonitorSource = source;

        this.WhenAnyValue(
                e => e.MonitorSource.Device.AttachedDisplay.CurrentMode,
                (m) => m.Pels.Width)
            .ToProperty(this, e => e.Width,out _width);

        this.WhenAnyValue(
                e => e.MonitorSource.Device.AttachedDisplay.CurrentMode,
                (m) => m.Pels.Height)
            .ToProperty(this, e => e.Height,out _height);

        this.WhenAnyValue(
                e => e.MonitorSource.Device.AttachedDisplay.CurrentMode.Position,
                (Point p) => p.X)
            .ToProperty(this, e => e.X,out _x);

        this.WhenAnyValue(
                e => e.MonitorSource.Device.AttachedDisplay.CurrentMode.Position,
                (Point p) => p.Y)
            .ToProperty(this, e => e.Y,out _y);

    }

    public override double Width
    {
        get => _width.Get();
        set => throw new NotImplementedException();
    }
    readonly ObservableAsPropertyHelper<double> _width;

    // Monitor area was found depending on system scale

    //private IProperty<double> _width = H.Property<double>(c => c
    //    .Set(e => e.Screen.Monitor.MonitorArea.Width)
    //    .On(e => e.Screen.Monitor.MonitorArea)
    //    .Update()
    //);

    public override double Height
    {
        get => _height.Get();
        set => throw new NotImplementedException();
    }
    readonly ObservableAsPropertyHelper<double> _height;

    //private IProperty<double> _height = H.Property<double>(c => c
    //    .Set(e => e.Screen.Monitor.MonitorArea.Height)
    //    .On(e => e.Screen.Monitor.MonitorArea)
    //    .Update()
    //);

    public override double X
    {
        get => _x.Get();
        set => throw new NotImplementedException();
    }
    readonly ObservableAsPropertyHelper<double> _x;
    //TODO : 
    //private readonly IProperty<double> _x = H.Property<double>(c => c
    //    .Set(s => s.Screen.Monitor.MonitorArea.X)
    //    .On( e => e.Screen.Monitor.MonitorArea)
    //    .Update()
    //);

    public override double Y
    {
        get => _y.Get();
        set => throw new NotImplementedException();
    }
    readonly ObservableAsPropertyHelper<double> _y;

   //private readonly IProperty<double> _y = H.Property<double>(nameof(Y), c => c
    //    .Set(s => s.Screen.Monitor.MonitorArea.Y)
    //    .On(e => e.Screen.Monitor.MonitorArea)
    //    .Update()
    //);


    public override double TopBorder
    {
        get => 0;
        set => throw new NotImplementedException();
    }
    public override double BottomBorder
    {
        get => 0;
        set => throw new NotImplementedException();
    }
    public override double LeftBorder
    {
        get => 0;
        set => throw new NotImplementedException();
    }
    public override double RightBorder
    {
        get => 0;
        set => throw new NotImplementedException();
    }

    double LoadValueMonitor(Func<double> def, [CallerMemberName] string name = null)
    {
        using (RegistryKey key = MonitorSource.Device.OpenMonitorRegKey())
        {
            return key.GetKey(name, def);
        }
    }

    double LoadValueConfig(Func<double> def, [CallerMemberName] string name = null)
    {
        using (RegistryKey key = MonitorSource.Monitor.OpenRegKey())
        {
            return key.GetKey(name, def);
        }
    }
}
