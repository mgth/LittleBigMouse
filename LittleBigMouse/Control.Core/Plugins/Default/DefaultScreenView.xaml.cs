using System.Windows.Controls;
using Hlab.Base;
using Hlab.Mvvm;

namespace LittleBigMouse.Control.Core.Plugins.Default
{
    //class DefaultScreenContentView : UserControl, IView<ViewModeDefault, LocationScreenViewModel>, IViewScreenFrameTopLayer
    //{
    //}
    /// <summary>
    /// Logique d'interaction pour LocationScreenView.xaml
    /// </summary>
    partial class DefaultScreenView : UserControl, IView<ViewModeDefault, DefaultScreenViewModel>, IViewScreenFrameContent
    {
    

        public DefaultScreenView() 
        {
            InitializeComponent();
        }



         private DefaultScreenViewModel ViewModel => (DataContext as DefaultScreenViewModel);
        MultiScreensView Presenter => this.FindParent<MultiScreensView>();
    }

}
