using System.Windows;

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

        private void ApplicationUpdateView_DataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            if(DataContext is ApplicationUpdateViewModel viewModel) viewModel.PropertyChanged += ViewModel_PropertyChanged;
        }

        private void ViewModel_PropertyChanged(object sender, System.ComponentModel.PropertyChangedEventArgs e)
        {
            if (e.PropertyName == "Updated" && ((DataContext as ApplicationUpdateViewModel)?.Updated??false))
            {
                Close();
            }
        }

        private void ButtonBase_OnClick(object sender, RoutedEventArgs e)
        {
           Close();
        }
    }
}
