/*
  HLab.Windows.MonitorVcp
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.MonitorVcp.

    HLab.Windows.MonitorVcp is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.MonitorVcp is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Collections.Generic;
using HLab.Mvvm.ReactiveUI;
using HLab.Sys.Windows.Monitors;
using ReactiveUI;

namespace HLab.Sys.Windows.MonitorVcp;

public class LuminanceViewModel : ViewModel
{
    public LuminanceViewModel()
    {
        _value  = ValueDefault();
    }

    public ISystemMonitorsService Monitors { get; set; }

    double ValueDefault()
    {
        double l = 0;

        if (Monitors == null) return l;

        foreach (var monitor in Monitors.Root.AllMonitorDevices())
        {
            var lut = monitor.ProbeLut(); 
            lut.Load();
            l = Math.Max(l, lut.Luminance);
        }

        return l;
    }

    public double Value
    {
        get => _value;

        set
        {
            if (Monitors == null || Math.Abs(_value - value) < double.Epsilon) return;

            using (DelayChangeNotifications())
            {
                this.RaiseAndSetIfChanged(ref _value, value);
                foreach (var monitor in Monitors.Root.AllMonitorDevices())
                {
                    var lut = monitor.ProbeLut();
                    //lut.Load();
                    lut.Luminance = value;
                }

            }
        }
    }
    double _value;

    public double Max
    {
        get
        {
            if (Monitors == null) return 0;

            double max = 0;

            foreach (var monitor in Monitors.Root.AllMonitorDevices())
            {
                var lut = monitor.ProbeLut();
                lut.Load();
                max = Math.Max(max, lut.MaxLuminance);
            }
            return max;
        }
    }

    public double MaxAll
    {
        get
        {
            if (Monitors == null) return double.MaxValue;

            var max = double.MaxValue;

            foreach (var monitor in Monitors.Root.AllMonitorDevices())
            {
                var lut = monitor.ProbeLut();
                lut.Load();
                max = Math.Min(max, lut.MaxLuminance);
            }

            return max;
        }
    }

    public double Min
    {
        get
        {
            if (Monitors == null) return double.MaxValue;

            var min = double.MaxValue;

            foreach (var monitor in Monitors.Root.AllMonitorDevices())
            {
                var lut = monitor.ProbeLut();
                lut.Load();
                min = Math.Min(min, lut.MinLuminance);
            }
            return min;
        }
    }
    public double MinAll
    {
        get
        {
            if (Monitors == null) return 0;

            double min = 0;

            foreach (var monitor in Monitors.Root.AllMonitorDevices())
            {
                var lut = monitor.ProbeLut();
                lut.Load();
                min = Math.Max(min, lut.MinLuminance);
            }

            return min;
        }
    }

    // TODO 
    //public Brush CursorBrush => new SolidColorBrush(AccentColorSet.ActiveSet["SystemAccent"]); //);
    //public Brush TrackBrush => new SolidColorBrush(AccentColorSet.ActiveSet["SystemAccentLight1"]); //);
    //public Brush TickBrush => new SolidColorBrush(Colors.White);//AccentColorSet.ActiveSet["ControlScrollbarThumbBorderPressed"]);

    public IEnumerable<double> Ticks
    {
        get
        {
            if (Monitors is not  null)
                foreach (var monitor in Monitors.Root.AllMonitorDevices())
                {
                    var lut = monitor.ProbeLut();
                    lut.Load();
                    yield return lut.MinLuminance;
                    yield return lut.MaxLuminance;
                }
        }
    }


}