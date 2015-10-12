using System;
using System.ComponentModel;
using System.Threading;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace LittleBigMouse
{
    /// <summary>
    /// Logique d'interaction pour ControlPane.xaml
    /// </summary>
    public partial class ControlPane : UserControl, IPropertyPane
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private void Changed(string name)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
        }

        public ControlPane()
        {
            InitializeComponent();
            DataContext = this;
        }
        private Screen _screen = null;
        public Screen Screen
        {
            get { return _screen; }
            set
            {
                _screen = value;
                ShowPattern();

                Changed("Screen");
            }
        }


        private TestPatternWindow _pattern;
        private void ShowPattern()
        {
            if (_pattern == null) return;

            if (Screen != null)
            {
                _pattern.Left = Screen.Bounds.TopLeft.DpiAware.X;
                _pattern.Top = Screen.Bounds.TopLeft.DpiAware.Y;
                _pattern.Width = Screen.Bounds.BottomRight.DpiAware.X - Screen.Bounds.TopLeft.DpiAware.X;
                _pattern.Height = Screen.Bounds.BottomRight.DpiAware.Y - Screen.Bounds.TopLeft.DpiAware.Y;

                _pattern.Show();
            }
            else
            {
                _pattern.Hide();
            }
        }

        private void ShowPattern(TestPatternType type, Color color = default(Color))
        {
            if (_pattern == null) _pattern = new TestPatternWindow();

                _pattern.PatternType = type;
                _pattern.PatternColor = color;

                ShowPattern();
       }



        private void pattern_Click(object sender, RoutedEventArgs e)
        {
            TestPatternButton b = sender as TestPatternButton;
            if (b != null)
                ShowPattern(b.PatternType, b.PatternColor);
        }

        private void cmdProbe_Click(object sender, RoutedEventArgs e)
        {
            Thread thread = new Thread(
            new ThreadStart(
              Tune));

            thread.Start();
        }

        private void Tune()
        {
            int count = 3;
            uint channel = 0;

            while (count > 0)
            {
                if (Dicotune(Screen.Gain.Channel(channel))) count = 2;
                else count--;

                channel++;
                if (channel == 3) channel = 0;
            }
        }

        private bool Dicotune(MonitorLevel gain)
        {
            uint oldGain = gain.Value;

            uint max = gain.Max;
            uint min = gain.Min;

            while (min != max)
            {
                uint h = Math.Max(1, (max - min) / 3);

                gain.Value = min + h;
                double devMin = ProbeDev();

                gain.Value = max - h;
                double devMax = ProbeDev();

                if (devMin > devMax) min += h;
                else max -= h;
            }


            return (min != oldGain);
        }

        private bool Tune(MonitorLevel gain)
        {
            uint oldGain = gain.Value;

            double olddev;
            double dev = ProbeDev();
            do
            {
                olddev = dev;
                gain.Value--;
                dev = ProbeDev();
            } while (dev < olddev && gain.Value > gain.Min);

            if (dev > olddev) gain.Value++;

            dev = ProbeDev();
            do
            {
                olddev = dev;
                gain.Value++;
                dev = ProbeDev();
            } while (dev < olddev && gain.Value < gain.Max);

            if (dev > olddev) gain.Value--;

            return (oldGain != gain.Value);
        }


        private double ProbeDev()
        {
            Argyll probe = new Argyll();

            Screen.ProbedColor = new ProbedColor(probe.spotread());

            return Screen.ProbedColor.DeviationRGB();
        }


        private Curve _curve;
        public Curve Curve
        {
            get { return _curve; }
            set
            {
                _curve = value;
                Changed("Curve");
            }
        }



        private void ProbeLevel(MonitorLevel level, Curve c)
        {
            Argyll probe = new Argyll();

            for (uint i = level.Min; i <= level.Max; i++)
            {
                level.Value = i;
                Screen.ProbedColor = new ProbedColor(probe.spotread());
                c[i - level.Min] = Screen.ProbedColor.Luminance;
            }

        }
        private void ProbeScale(Color color, Curve c)
        {
            Argyll probe = new Argyll();

            TestPatternWindow pattern = new TestPatternWindow();


            for (double i = 0; i <= 50; i++)
            {

                pattern.PatternType = TestPatternType.Solid;
                pattern.PatternColor = Color.FromRgb(
                    (byte)(i * (double)color.R / 256),
                    (byte)(i * (double)color.G / 256),
                    (byte)(i * (double)color.B / 256)
                    );

                pattern.ShowOnScreen(Screen);

                //System.Windows.Threading.Dispatcher.Run();
                Screen.ProbedColor = new ProbedColor(probe.spotread());
                c[(uint)i] = Screen.ProbedColor.Luminance;

                pattern.Close();
                pattern = new TestPatternWindow();
            }
        }

        private void cmdProbeBrightness_Click(object sender, RoutedEventArgs e)
        {
            Curve c = new Curve((int)Screen.Brightness.Max + 1, Colors.White);
            curve.AddCurve(c);

            Thread thread = new Thread(
                new ThreadStart(
                  delegate ()
                  {
                      ProbeLevel(Screen.Brightness,c);
                  }));

            thread.Start();

        }

        private void cmdProbeContrast_Click(object sender, RoutedEventArgs e)
        {
            Curve c = new Curve((int)Screen.Contrast.Max + 1, Colors.Yellow);
            curve.AddCurve(c);

            Thread thread = new Thread(
                new ThreadStart(
                  delegate ()
                  {
                      ProbeLevel(Screen.Contrast,c);
                  }));

            thread.Start();

        }

        private void cmdProbeGrayScale_Click(object sender, RoutedEventArgs e)
        {
            Curve c = new Curve(256, Colors.Red);
            curve.AddCurve(c);

            Thread thread = new Thread(
                new ThreadStart(
                  delegate ()
                  {
                      if (_pattern!=null)
                          ProbeScale(_pattern.PatternColor, c);
                      else
                          ProbeScale(Colors.White, c);
                  }));
            thread.SetApartmentState(ApartmentState.STA);
            thread.Start();

        }

        private void cmdProbeRedGain_Click(object sender, RoutedEventArgs e)
        {
            Curve c = new Curve((int)Screen.Gain.Red.Max + 1, Colors.Red);
            curve.AddCurve(c);

            Thread thread = new Thread(
                new ThreadStart(
                  delegate ()
                  {
                      ProbeLevel(Screen.Gain.Red, c);
                  }));

            thread.Start();

        }
        private void cmdProbeGreenGain_Click(object sender, RoutedEventArgs e)
        {
            Curve c = new Curve((int)Screen.Gain.Green.Max + 1, Colors.Green);
            curve.AddCurve(c);

            Thread thread = new Thread(
                new ThreadStart(
                  delegate ()
                  {
                      ProbeLevel(Screen.Gain.Green, c);
                  }));

            thread.Start();

        }
        private void cmdProbeBlueGain_Click(object sender, RoutedEventArgs e)
        {
            Curve c = new Curve((int)Screen.Gain.Blue.Max + 1, Colors.Blue);
            curve.AddCurve(c);

            Thread thread = new Thread(
                new ThreadStart(
                  delegate ()
                  {
                      ProbeLevel(Screen.Gain.Blue, c);
                  }));

            thread.Start();

        }

    }
}
