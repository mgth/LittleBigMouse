using System;
using System.Windows.Media;
using LbmScreenConfig;
using NotifyChange;

namespace MonitorVcp
{
    class LuminanceViewModel : Notifier
    {
        public ScreenConfig Config { get; set; }


        private double _luminance = -1;
        public double Value
        {
            get
            {
                if (_luminance >= 0) return _luminance;

                double l = 0;

                if (Config == null) return l;

                foreach (Screen screen in Config.AllScreens)
                {
                    ProbeLut lut = screen.ProbeLut(); //ProbeLut.GetLut(screen);
                    lut.Load();
                    l = Math.Max(l, lut.Luminance);
                }

                _luminance = l;
                return _luminance;
            }

            set
            {
                if (Config != null)
                    if (SetProperty(ref _luminance, value))
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
