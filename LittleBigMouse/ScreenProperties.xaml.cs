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

namespace LittleBigMouse
{
    /// <summary>
    /// Logique d'interaction pour ScreenProperties.xaml
    /// </summary>
    public partial class ScreenProperties : UserControl
    {
        private Screen _screen;
        public ScreenProperties(Screen s)
        {
            _screen = s;
            InitializeComponent();

            DataContext = _screen;
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            _screen.DpiX = double.NaN;
            _screen.DpiY = double.NaN;
        }
    }
}
