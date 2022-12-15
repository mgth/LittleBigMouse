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
using System.Windows.Media;
using HLab.Base.Wpf;
using HLab.Base.Wpf.Themes;
using HLab.Mvvm;
using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.Monitors;

namespace HLab.Sys.Windows.MonitorVcp
{
    using H = H<LuminanceViewModel>;

    class LuminanceViewModel : ViewModel
    {
        public LuminanceViewModel()
        {
            H.Initialize(this);
        }


        public IMonitorsService Monitors { get; set; }


        private double ValueDefault()
        {
                double l = 0;

                if (Monitors == null) return l;

                foreach (var monitor in Monitors.AttachedMonitors.Items)
                {
                    var lut = monitor.ProbeLut(); //ProbeLut.GetLut(screen);
                    lut.Load();
                    l = Math.Max(l, lut.Luminance);
                }


                return l;
         }

        private readonly IProperty<double> _value = H.Property<double>(c => c
            .Set(e => e.ValueDefault()));
        public double Value
        {
            get => _value.Get();

            set
            {
                if (Monitors != null)
                    _value.Set(value,(v)=>
                    {
                        foreach (var monitor in Monitors.AttachedMonitors.Items)
                        {
                            var lut = monitor.ProbeLut();
                            //lut.Load();
                            lut.Luminance = value;
                        }
                    });
            }
        }
        public double Max
        {
            get
            {
                double max = 0;

                if (Monitors != null)
                    foreach (var monitor in Monitors.AttachedMonitors.Items)
                    {
                        ProbeLut lut = monitor.ProbeLut();
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
                double max = double.MaxValue;

                if (Monitors != null)
                    foreach (var monitor in Monitors.AttachedMonitors.Items)
                    {
                        ProbeLut lut = monitor.ProbeLut();
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
                double min = double.MaxValue;

                if (Monitors!=null)
                foreach (var monitor in Monitors.AttachedMonitors.Items)
                {
                    ProbeLut lut = monitor.ProbeLut();
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
                double min = 0;

                if (Monitors != null)
                    foreach (var monitor in Monitors.AttachedMonitors.Items)
                    {
                        ProbeLut lut = monitor.ProbeLut();
                        lut.Load();
                        min = Math.Max(min, lut.MinLuminance);
                    }

                return min;
            }
        }

        public Brush CursorBrush => new SolidColorBrush(AccentColorSet.ActiveSet["SystemAccent"]); //);
        public Brush TrackBrush => new SolidColorBrush(AccentColorSet.ActiveSet["SystemAccentLight1"]); //);
        public Brush TickBrush => new SolidColorBrush(Colors.White);//AccentColorSet.ActiveSet["ControlScrollbarThumbBorderPressed"]);

        public DoubleCollection Ticks
        {
            get
            {
                var ticks = new DoubleCollection();
                if (Monitors == null) return ticks;

                foreach (var monitor in Monitors.AttachedMonitors.Items)
                {
                    var lut = monitor.ProbeLut();
                    lut.Load();
                    ticks.Add(lut.MinLuminance);
                    ticks.Add(lut.MaxLuminance);
                }
                return ticks;
            }
        }


    }
}
