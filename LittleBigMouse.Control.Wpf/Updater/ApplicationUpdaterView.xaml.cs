using System.Windows;
using LittleBigMouse.Control.Updater;

namespace LittleBigMouse_Daemon.Updater
{
    /// <summary>
    /// Logique d'interaction pour ApplicationUpdate.xaml
    /// </summary>
    public partial class ApplicationUpdateView : Window
    {
        public ApplicationUpdateView()
        {
            InitializeComponent();
            DataContextChanged += ApplicationUpdateView_DataContextChanged;
        }

        void ApplicationUpdateView_DataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            if(DataContext is ApplicationUpdateViewModel viewModel) viewModel.PropertyChanged += ViewModel_PropertyChanged;
        }

        void ViewModel_PropertyChanged(object sender, System.ComponentModel.PropertyChangedEventArgs e)
        {
            if (e.PropertyName == "Updated" && ((DataContext as ApplicationUpdateViewModel)?.Updated??false))
            {
                Close();
            }
        }

        void ButtonBase_OnClick(object sender, RoutedEventArgs e)
        {
           Close();
        }
    }
}
