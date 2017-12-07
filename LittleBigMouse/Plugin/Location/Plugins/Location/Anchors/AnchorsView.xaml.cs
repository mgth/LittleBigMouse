using System.Windows.Controls;
using Hlab.Mvvm;
using LittleBigMouse.Control.Core;

namespace LittleBigMouse.LocationPlugin.Plugins.Location.Anchors
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
