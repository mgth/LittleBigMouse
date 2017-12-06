using System.Windows.Controls;
using LittleBigMouse.ControlCore;
using Erp.Mvvm;

namespace LittleBigMouse.LocationPlugin.Plugins.Location
{
    /// <summary>
    /// Logique d'interaction pour AnchorsView.xaml
    /// </summary>
    public partial class AnchorsView : UserControl , IView<ViewModeMultiScreenBackgound, AnchorsViewModel>
    {
        public AnchorsView()
        {
            InitializeComponent();
        }
    }
}
