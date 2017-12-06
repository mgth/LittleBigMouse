using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using Erp.Base;
using Erp.Mvvm;
using LittleBigMouse.ControlCore;
using LittleBigMouse.LocationPlugin.Plugins.Location;

namespace LittleBigMouse.LocationPlugin.Plugins.Default
{
    class DefaultScreenContentView : UserControl, IView<ViewModeScreenLocation, LocationScreenViewModel>, IViewScreenFrameTopLayer
    {
    }
    /// <summary>
    /// Logique d'interaction pour LocationScreenView.xaml
    /// </summary>
    partial class DefaultScreenView : UserControl, IView<ViewModeDefault, DefaultScreenViewModel>, IViewScreenFrameContent
    {
    

        public DefaultScreenView() 
        {
            InitializeComponent();
        }



         private LocationScreenViewModel ViewModel => (DataContext as LocationScreenViewModel);
        MultiScreensView Presenter => this.FindParent<MultiScreensView>();
    }

}
