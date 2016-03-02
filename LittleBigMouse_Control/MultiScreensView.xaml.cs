using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;
using LbmScreenConfig;
using NotifyChange;

namespace LittleBigMouse_Control
{
    /// <summary>
    /// Logique d'interaction pour MultiScreensGui.xaml
    /// </summary>
    public partial class MultiScreensView : UserControl
    {
        public MultiScreensView()
        {
            InitializeComponent();
        }

        private void ScreensPresenter_MouseDown(object sender, MouseButtonEventArgs e)
        {
            var presenter = DataContext as MultiScreensViewModel;
            var view = presenter?.MainViewModel.View as MainView;
            view?.OnMouseDown(sender,e);
        }
    }
}
