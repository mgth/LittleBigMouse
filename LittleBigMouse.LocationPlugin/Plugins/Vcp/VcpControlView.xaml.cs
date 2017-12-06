using System.Collections.Generic;
using System.Windows;
using System.Windows.Controls;
using LbmScreenConfig;
using LittleBigMouse_Control.Plugins.Vcp;
using Erp.Mvvm;
using LittleBigMouse.ControlCore;
using LittleBigMouse.LocationPlugin.Plugins.Location.Rulers;

namespace LittleBigMouse.LocationPlugin.Plugins.Vcp
{
    /// <summary>
    /// Logique d'interaction pour ControlGuiSizer.xaml
    /// </summary>
    public partial class VcpControlView : UserControl, IView<ViewModeScreenVcp, ScreenVcpViewModel>
    {

        public VcpControlView()
        {
            InitializeComponent();
        }

        private void Save()
        {
 //           Model.Save();
        }


        private void cmdApply_Click(object sender, RoutedEventArgs e)
        {
            Save();
            //LittleBigMouseClient.Client.LoadAtStartup(Model.LoadAtStartup);
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



        private readonly List<RulerView> _rulers = new List<RulerView>();
        private bool _liveUpdate = false;



        private void ButtonBase_OnClick(object sender, RoutedEventArgs e)
        {
            ;
        }


    }
}