using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.ServiceModel.Channels;
using System.Threading;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using LbmScreenConfig;
using OxyPlot;
using OxyPlot.Series;
using WinAPI_Dxva2;

namespace LittleBigMouse_Control.VcpPlugin
{
    /// <summary>
    /// Logique d'interaction pour ScreenGuiSizer.xaml
    /// </summary>
    public partial class ScreenGuiVcp : ScreenGuiControl
    {
        public ScreenVcp Vcp { get; }
        public ScreenGuiVcp(Screen screen):base(screen)
        {
            Vcp = new ScreenVcp(screen);

            InitializeComponent();

            BrightnessSlider.MonitorLevel = Vcp.Brightness;
            ContrastSlider.MonitorLevel = Vcp.Contrast;

            RedSlider.MonitorLevel = Vcp.Gain.Red;
            GreenSlider.MonitorLevel = Vcp.Gain.Green;
            BlueSlider.MonitorLevel = Vcp.Gain.Blue;

            DataContext = this;
        }

        private void ButtonOff_OnClick(object sender, RoutedEventArgs e)
        {
            Dxva2.SetVCPFeature(Screen.HPhysical, 0xD6, 4);
        }
        private void WakeUp_OnClick(object sender, RoutedEventArgs e)
        {
                //Dxva2.SetVCPFeature(ScreenGui.Screen.HPhysical, 0x60, 0x0f);
                Dxva2.SetVCPFeature(Screen.HPhysical, 0xD6, 1);
                //Dxva2.SetVCPFeature(ScreenGui.Screen.HPhysical, 0xE1, 1);
                Dxva2.SetVCPFeature(Screen.HPhysical, 0xE1, 0);              
        }
        //private void ButtonOn_OnClick(object sender, RoutedEventArgs e)
        //{
        //    uint code = Convert.ToUInt32(txtCode.Text, 16);
        //    uint value = Convert.ToUInt32(txtValue.Text, 16);

        //    uint pvct;
        //    uint current;
        //    uint max;

        //    Dxva2.GetVCPFeatureAndVCPFeatureReply(ScreenGui.Screen.HPhysical, code, out pvct, out current, out max);

        //    Debug.Print(pvct.ToString() + ":" + current.ToString() + "<" + max.ToString());

        //    Dxva2.SetVCPFeature(ScreenGui.Screen.HPhysical, code, value);
        //    //for (uint i = 0; i < max; i++)
        //    //{
        //    //    if (i==5 && code==0xD6) continue; 
        //    //    bool result = Dxva2.SetVCPFeature(Screen.HPhysical, code, i);
        //    //    Debug.Print(i.ToString() + (result?":O":"X"));
        //    //}

        //    //IntPtr desk = User32.GetDesktopWindow();
        //    //IntPtr win = User32.FindWindowEx(IntPtr.Zero, IntPtr.Zero, null, null);

        //    //User32.SendMessage(-1, User32.WM_SYSCOMMAND, User32.SC_MONITORPOWER, 2);
        //    //User32.SendMessage(-1, User32.WM_SYSCOMMAND, User32.SC_MONITORPOWER, -1);
        //}

        private void Probe_OnClick(object sender, RoutedEventArgs e)
        {
            var probe = new Argyll.LittleBigMouse.Argyll();
            //Vcp.Brightness.SetToMax();
            //Vcp.Contrast.SetToMax();
            uint red = Vcp.Gain.Red.Value;
            uint green = Vcp.Gain.Green.Value;
            uint blue = Vcp.Gain.Blue.Value;
            //Vcp.Brightness.SetToMax();
            //Vcp.Contrast.SetToMax();
            Vcp.Gain.Red.SetToMax();
            Vcp.Gain.Green.SetToMax();
            Vcp.Gain.Blue.SetToMax();
            new Thread(() =>
            {
                MonitorLevel level = Vcp.Contrast;
                LineSeries _line = new LineSeries();

                //_line.Color = OxyColors.Red;

                Curve.PlotModel.Series.Add(_line);

                for (uint i = level.Min; i <= level.Max; i++)
                {
                    level.Value = i;

                    ProbedColor color = probe.SpotRead();

                    _line.Points.Add(new DataPoint(i, color.Lab.L));
                    Curve.Refresh();
                }

                Vcp.Gain.Red.Value = red;
                Vcp.Gain.Green.Value = green;
                Vcp.Gain.Blue.Value = blue;

            }
            ).Start();
            return;

            new Thread (() => {
                                  for (uint channel = 0; channel < 3; channel++)
                                  {
                                    MonitorLevel level = Vcp.Gain.Channel(channel);
                                    LineSeries line = new LineSeries();

                                      switch (channel)
                                      {
                        case 0:
                            line.Color = OxyColors.Red;
                                              break;
                        case 1:
                                              line.Color = OxyColors.Green;
                                              break;
                                          case 2:
                                              line.Color = OxyColors.Blue;
                                              break;
                                      }

                                    Curve.PlotModel.Series.Add(line);

                                      uint max = level.Value;

                                    for (uint i = 32; i <= max; i++)
                                    {
                                        level.Value = i; Thread.Sleep(1000);

                                        ProbedColor color = probe.SpotRead();

                                        line.Points.Add(new DataPoint(i,color.DeltaE00()));
                                        Curve.Refresh();
                                    }

                                      double min = double.MaxValue;
                                      uint minIdx = level.Value;
                                      foreach (DataPoint dp in line.Points)
                                      {
                                          if (dp.Y < min)
                                          {
                                              min = dp.Y;
                                              minIdx = (uint)dp.X;
                                          }
                                      }

                                      level.Value = minIdx;
                                  }
            }
            ).Start();
        }

        private void Tune_OnClick(object sender, RoutedEventArgs e)
        {
            new Thread(Tune).Start();

        }

        private double[,,] _tune;
        private LineSeries _line;
        public void Tune()
        {
            _line = new LineSeries();
            Curve.PlotModel.Series.Add(_line);

            int count = 6;
            uint channel = 0;

            _tune = new double[Vcp.Gain.Red.Max+1, Vcp.Gain.Green.Max+1, Vcp.Gain.Blue.Max+1];

            uint[] rgb = {Vcp.Gain.Red.Value, Vcp.Gain.Green.Value, Vcp.Gain.Blue.Value};

            while (count > 0)
            {
                if (Tune(channel,ref rgb)) count = 5;
                else count--;

                channel++;
                channel %= 6;
            }

            while (Vcp.Gain.Red.Value != rgb[0]) Vcp.Gain.Red.Value = rgb[0];
            while (Vcp.Gain.Green.Value != rgb[1]) Vcp.Gain.Green.Value = rgb[1];
            while (Vcp.Gain.Blue.Value != rgb[2]) Vcp.Gain.Blue.Value = rgb[2];
        }

        private bool Tune(uint channel,ref uint[] rgb)
        {
            uint c1 = channel%3;
            uint c2;
            switch (channel)
            {
                case 3: //0
                    c2 = 1;
                    break;
                case 4://1
                    c2 = 2;
                    break;
                case 5://2
                    c2 = 0;
                    break;
                default:
                    c2 = 3;
                    break;
            }

            uint min = Vcp.Gain.Channel(c1).Min;
            uint max = Vcp.Gain.Channel(c1).Max;

            uint min2 = 0;
            uint max2 = 0;

            if (c2 < 3)
            {
                min2 = Vcp.Gain.Channel(c2).Min;
                max2 = Vcp.Gain.Channel(c2).Max;                
            }

            uint oldGain = rgb[c1];
            //uint old2 = rgb[c2];

            double deltaE = Probe(rgb);
            _line.Points.Add(new DataPoint(_line.Points.Count, deltaE));
            Curve.Refresh();

            while (rgb[c1] > min && (c2==3 || rgb[c2] > min))
            {
                double old = deltaE;
                rgb[c1] -= 1;
                if(c2<3) rgb[c2] -= 1;

                deltaE = Probe(rgb);

                if (deltaE > old) break;

                _line.Points.Add(new DataPoint(_line.Points.Count,deltaE));
                Curve.Refresh();
            }

            while (rgb[c1] < max && (c2 == 3 || rgb[c2] < max))
            {
                double old = deltaE;
                rgb[c1] ++;
                if (c2 < 3) rgb[c2] ++;

                deltaE = Probe(rgb);

                if (deltaE > old)
                {
                    rgb[c1]--;
                    if (c2 < 3) rgb[c2]--;
                    deltaE = Probe(rgb);
                    break;
                }

                _line.Points.Add(new DataPoint(_line.Points.Count, deltaE));
                Curve.Refresh();
            }

            return (oldGain != rgb[c1]);
    }
        private double Probe(uint[] rgb)
        {
            if (_tune[rgb[0], rgb[1], rgb[2]] == 0)
            {
                while(Vcp.Gain.Red.Value != rgb[0]) Vcp.Gain.Red.Value = rgb[0];
                while(Vcp.Gain.Green.Value != rgb[1]) Vcp.Gain.Green.Value = rgb[1];
                while(Vcp.Gain.Blue.Value != rgb[2]) Vcp.Gain.Blue.Value = rgb[2];             

                Thread.Sleep(500);

                Argyll.LittleBigMouse.Argyll probe = new Argyll.LittleBigMouse.Argyll();
                ProbedColor color = probe.SpotRead();
 
                //return color.DeltaE00();
                _tune[rgb[0], rgb[1], rgb[2]] = color.DeltaE00();
            }
            return _tune[rgb[0], rgb[1], rgb[2]];
        }
        private void UIElement_OnMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
        }
    }
}
