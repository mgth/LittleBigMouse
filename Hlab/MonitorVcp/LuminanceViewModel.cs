/*
  HLab.Windows.MonitorVcp
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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
using System.ComponentModel;
using System.Windows.Media;
using Hlab.Notify;
using LbmScreenConfig;
using MonitorVcp;

namespace Hlab.Windows.MonitorVcp
{
    class LuminanceViewModel : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }
        public ScreenConfig Config { get; set; }


        private double ValueDefault()
        {
                double l = 0;

                if (Config == null) return l;

                foreach (Screen screen in Config.AllScreens)
                {
                    ProbeLut lut = screen.ProbeLut(); //ProbeLut.GetLut(screen);
                    lut.Load();
                    l = Math.Max(l, lut.Luminance);
                }


                return l;
         }

        public double Value
        {
            get => this.Get(ValueDefault);

            set
            {
                if (Config != null)
                    if (this.Set(value))
                    {
                        foreach (Screen screen in Config.AllScreens)
                        {
                            ProbeLut lut = screen.ProbeLut();
                            //lut.Load();
                            lut.Luminance = value;
                        }
                    }
            }
        }
        public double Max
        {
            get
            {
                double max = 0;

                if (Config != null)
                    foreach (Screen screen in Config.AllScreens)
                {
                    ProbeLut lut = screen.ProbeLut();
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

                if (Config != null)
                    foreach (Screen screen in Config.AllScreens)
                {
                    ProbeLut lut = screen.ProbeLut();
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

                if (Config!=null)
                foreach (Screen screen in Config.AllScreens)
                {
                    ProbeLut lut = screen.ProbeLut();
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

                if (Config != null)
                    foreach (Screen screen in Config.AllScreens)
                {
                    ProbeLut lut = screen.ProbeLut();
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
                DoubleCollection ticks = new DoubleCollection();
                if (Config != null)
                    foreach (Screen screen in Config.AllScreens)
                {
                    ProbeLut lut = screen.ProbeLut();
                    lut.Load();
                    ticks.Add(lut.MinLuminance);
                    ticks.Add(lut.MaxLuminance);
                }
                return ticks;
            }
        }


    }
}
