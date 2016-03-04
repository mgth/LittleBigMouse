using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using LbmScreenConfig;

namespace LittleBigMouse_Control.VcpPlugin
{
    /// <summary>
    /// Logique d'interaction pour ControlGuiSizer.xaml
    /// </summary>
    public partial class VcpControlView : UserControl
    {

        public VcpControlView()
        {
            InitializeComponent();
            DataContext = this;
        }

        private void Save()
        {
 //           Config.Save();
        }


        private void cmdApply_Click(object sender, RoutedEventArgs e)
        {
            Save();
            //LittleBigMouseClient.Client.LoadAtStartup(Config.LoadAtStartup);
            LittleBigMouseClient.Client.LoadConfig();
            LittleBigMouseClient.Client.Start();
        }

        private void cmdCancel_Click(object sender, RoutedEventArgs e)
        {
           // MainGui.Close();
        }


        private void cmdUnload_Click(object sender, RoutedEventArgs e)
        {
            LittleBigMouseClient.Client.Quit();
        }

        private bool _showRulers = false;



        private readonly List<Ruler> _rulers = new List<Ruler>();
        private bool _liveUpdate = false;



        private void ButtonBase_OnClick(object sender, RoutedEventArgs e)
        {
            ;
        }


    }
}