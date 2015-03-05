using System;
using System.Collections.Generic;
using System.ComponentModel;
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
    public partial class ScreenProperties : UserControl, PropertyPane, INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private void changed(String name)
        {
            if (PropertyChanged != null) PropertyChanged(this, new PropertyChangedEventArgs(name));
        }
        private Screen _screen = null;
        public Screen Screen
        {
            get { return _screen; }
            set {
                _screen = value;
                changed("Screen");
                changed("AllowEdit");
            }
        }
        public ScreenProperties()
        {
            InitializeComponent();
            DataContext = this;
        }

        public bool AllowEdit
        {
            get { return _screen != null; }
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            if (Screen!=null)
            {
                Screen.DpiX = double.NaN;
                Screen.DpiY = double.NaN;
            }
        }
    }
}
