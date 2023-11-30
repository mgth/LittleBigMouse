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

using System;
using System.Linq;
using System.Reactive.Linq;
using Avalonia.Controls;
using Avalonia.Media;
using DynamicData;
using HLab.Base.Avalonia.Extensions;
using HLab.Mvvm.ReactiveUI;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Plugins.Debug;
public struct MonitorDebugListValue
{
    public MonitorDebugListValue(string name, string value, Action? handler, bool isTitle)
    {
        Name = name;
        Value = value;
        Handler = handler;
        IsTitle = isTitle;
    }

    public string Name { get; init; }
    public string Value { get; init; }
    public Action? Handler { get; init; }
    public bool IsTitle { get; init; }
}

public class MonitorDebugViewModel : ViewModel<PhysicalMonitor>
{
    int _row = 0;
    readonly ISystemMonitorsService _monitors;

    public MonitorDebugViewModel(ISystemMonitorsService monitors)
    {
        _monitors = monitors;

        Grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Auto) });
        Grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        Grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Auto) });
    }

    protected override PhysicalMonitor? OnModelChanging(PhysicalMonitor? oldModel, PhysicalMonitor? newModel)
    {
        var device = _monitors.Root.AllChildren<MonitorDevice>()
        .FirstOrDefault(d => d.DeviceId == newModel?.IdPhysicalMonitor);

        device?.DisplayValues(AddValue);

        return base.OnModelChanging(oldModel, newModel);
    }


    public ISourceList<MonitorDebugListValue> Values { get; } = new SourceList<MonitorDebugListValue>();

    void AddValue(string name, string value, Action? handler = null, bool isTitle = false)
    {

        Values.Add(new MonitorDebugListValue
        {
            Name = name, 
            Value = value,
            Handler = handler,
            IsTitle = isTitle
        });

        
        Grid.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Auto) });

        var labelName = new Label { Content = name };
        var labelValue = new TextBox {
            Text = value,
            Background = isTitle?new SolidColorBrush(Colors.Gray):new SolidColorBrush(Color.FromArgb(0x80,0x00, 0x00, 0x00)),
        };

        if (handler != null)
        {
            var button = new Button {Content = "..."};
            button.Click += (o,a)=>handler();
            button.SetValue(Grid.ColumnProperty,2);
            button.SetValue(Grid.RowProperty, _row);
            Grid.Children.Add(button);
        }

        labelName.SetValue(Grid.ColumnProperty,0);
        labelValue.SetValue(Grid.ColumnProperty,1);

        labelName.Foreground = new SolidColorBrush(Colors.AntiqueWhite);
        labelValue.Foreground = new SolidColorBrush(Colors.White);

        labelName.SetValue(Grid.RowProperty, _row);
        labelValue.SetValue(Grid.RowProperty, _row);

        Grid.Children.Add(labelName);
        Grid.Children.Add(labelValue);

        _row++;
        
    }


    public Grid Grid { get; } = new Grid();

}