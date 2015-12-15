using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Windows;
using LbmScreenConfig;

namespace LittleBigMouse_Control.VcpPlugin
{
    /// <summary>
    /// Logique d'interaction pour ControlGuiSizer.xaml
    /// </summary>
    public partial class ControlGuiVcp : ControlGui
    {

        public ControlGuiVcp() : base()
        {
            InitializeComponent();
            DataContext = this;
        }

        private void Save()
        {
            Config.Save();
        }

        private void cmdOk_Click(object sender, RoutedEventArgs e)
        {
            cmdApply_Click(sender, e);
            MainGui.Close();
        }

        private void cmdApply_Click(object sender, RoutedEventArgs e)
        {
            Save();
            LittleBigMouseClient.Client.LoadAtStartup(Config.LoadAtStartup);
            LittleBigMouseClient.Client.LoadConfig();
            LittleBigMouseClient.Client.Start();
        }

        private void cmdCancel_Click(object sender, RoutedEventArgs e)
        {
            MainGui.Close();
        }


        private void cmdUnload_Click(object sender, RoutedEventArgs e)
        {
            LittleBigMouseClient.Client.Quit();
        }

        private bool _showRulers = false;



        private readonly List<Ruler> _rulers = new List<Ruler>();
        private bool _liveUpdate = false;

        private void AddRuler(RulerSide side)
        {
            if (Config.Selected == null) return;

            foreach (var sz in Config.AllScreens.Select(s => new Ruler(Config.Selected, s, side)))
            {
                _rulers.Add(sz);
            }
        }


        private void ButtonBase_OnClick(object sender, RoutedEventArgs e)
        {
            ;
        }

        public double Luminance
        {
            get
            {
                double l = 0;

                foreach (Screen screen in Config.AllScreens)
                {
                    ProbeLut lut = screen.ProbeLut(); //ProbeLut.GetLut(screen);
                    lut.Load();
                    l += lut.Current.Y;
                }

                return l/Config.AllScreens.Count;
            }

            set
            {
                foreach (Screen screen in Config.AllScreens)
                {
                    ProbeLut lut = screen.ProbeLut();
                    lut.Load();
                    lut.Luminance = value;
                }
            }
        }

        public double MaxLuminance
        {
            get
            {
                double max = 0;

                foreach (Screen screen in Config.AllScreens)
                {
                    ProbeLut lut = screen.ProbeLut();
                    lut.Load();
                    max = Math.Max(max, lut.MaxLuminance);
                }

                return max;
            }
        }
        public double MinLuminance
        {
            get
            {
                double min = 0;

                foreach (Screen screen in Config.AllScreens)
                {
                    ProbeLut lut = screen.ProbeLut();
                    lut.Load();
                    min = Math.Min(min, lut.MinLuminance);
                }

                return min;
            }
        }
    }
}