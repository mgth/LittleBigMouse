using System.Windows.Controls;
using System.Windows.Input;

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
