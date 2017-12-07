using System;
using System.Windows;

namespace LittleBigMouse.LocationPlugin.Plugins.Location.Rulers
{
    /// <summary>
    /// Logique d'interaction pour Tester.xaml
    /// </summary>
    public partial class Tester : Window
    {
        public Tester()
        {
            InitializeComponent();

            DataContextChanged += Tester_DataContextChanged;

            SizeChanged += Tester_SizeChanged;
            LocationChanged += Tester_LocationChanged;
            StateChanged += Tester_StateChanged;
        }

        private void Tester_DataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            SetValues();
            (e.NewValue as TesterViewModel).PropertyChanged += Tester_PropertyChanged;
        }

        private void Tester_PropertyChanged(object sender, System.ComponentModel.PropertyChangedEventArgs e)
        {
            SetBack();
        }

        public void SetBack()
        {
            Hide();
            var l = Vm.LeftInDip;
            var t = Vm.TopInDip;
            var w = Vm.WidthInDip;
            var h = Vm.HeightInDip;

            Left = l-1;
            Left = l;
            Top = t;
            //FinalWidth = w;
            //Height = h;
            Show();
        }

        private void Tester_StateChanged(object sender, EventArgs e)
        {
           // SetValues();
        }

        private void Tester_LocationChanged(object sender, EventArgs e)
        {
           // SetValues();
        }

        private TesterViewModel Vm => DataContext as TesterViewModel;

        private void Tester_SizeChanged(object sender, SizeChangedEventArgs e)
        {
           // SetValues();
        }

        private void SetValues()
        {
            Vm.TopInDip = Top;
            Vm.LeftInDip = Left;

            Vm.BottomInDip = Top + ActualHeight;
            Vm.RightInDip = Left + ActualWidth;

            if(ActualHeight>0) Vm.HeightInDip = ActualHeight;
            if(ActualWidth>0) Vm.WidthInDip = ActualWidth;
            
        }

        private void ButtonBase_OnClick(object sender, RoutedEventArgs e)
        {
            SetValues();
        }
    }
}
