using System.Collections.Generic;
using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using SystemColors1;
using Erp.Mvvm;
using LbmScreenConfig;
using LittleBigMouse.ControlCore;
using LittleBigMouse.LocationPlugin.Plugins.Location.Rulers;
using LittleBigMouse_Control.Rulers;

namespace LittleBigMouse.LocationPlugin.Plugins.Location
{
    /// <summary>
    /// Logique d'interaction pour ControlGuiSizer.xaml
    /// </summary>
    public partial class LocationControlView : UserControl
        , IView<ViewModeDefault, LocationControlViewModel>, IScreenControlView
    {

        public LocationControlView() 
        {
            InitializeComponent();
        }

        private ICommand Save = new RoutedCommand();

        private void cmdOk_Click(object sender, RoutedEventArgs e)
        {
            cmdApply_Click(sender, e);
           // MainGui.Close();
        }

        private void cmdApply_Click(object sender, RoutedEventArgs e)
        {
            //Save();
            //LittleBigMouseClient.Client.LoadAtStartup(Config.LoadAtStartup);
            LittleBigMouseClient.Client.LoadConfig();
            LittleBigMouseClient.Client.Start();
        }

        private void cmdCancel_Click(object sender, RoutedEventArgs e)
        {
          //  MainGui.Close();
        }

        private void cmdUnload_Click(object sender, RoutedEventArgs e)
        {
            LittleBigMouseClient.Client.Quit();
        }



        private readonly List<RulerView> _rulers = new List<RulerView>();
        private bool _liveUpdate = false;

        private void AddRuler(RulerViewModel.RulerSide side)
        {
            //if (Config.Selected == null) return;

            //foreach (var sz in Config.AllScreens.Select(s => new Ruler(Config.Selected, s, side)))
            //{
            //    _rulers.Add(sz);
            //}
        }

        public bool LiveUpdate
        {
            get => _liveUpdate; set
            {
                //if (Change.Set(ref _liveUpdate, value) && value)
                //    ActivateConfig();
                //else
                //{
                //    LittleBigMouseClient.Client.LoadConfig();
                //}
            }
        }

        public void ActivateConfig()
        {
            if (LiveUpdate)
            {
                //Save();
                LittleBigMouseClient.Client.LoadConfig();
            }

        }
        private void Config_PropertyChanged(object sender, PropertyChangedEventArgs e)
        {
            ActivateConfig();
        }

        private void cmdColors_Click(object sender, RoutedEventArgs e)
        {
            new ColorsWindow().Show();
        }
    }
}