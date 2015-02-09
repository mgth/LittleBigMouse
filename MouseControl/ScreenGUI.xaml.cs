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

namespace MouseControl
{
    /// <summary>
    /// Interaction logic for ScreenGUI.xaml
    /// </summary>
    public partial class ScreenGUI : UserControl
    {
        private Screen _screen;
        public Screen Screen {
            get { return _screen; }
            set {
                _screen = value;
                _screen.PhysicalChanged += _screen_PhysicalChanged;
                UpdateValues();
            }
        }

        private void _screen_PhysicalChanged(object sender, EventArgs e)
        {
            UpdateValues();
        }

        public void UpdateValues()
        {
                txtTop.Text = _screen.PhysicalBounds.Top.ToString();
                txtBottom.Text = _screen.PhysicalBounds.Bottom.ToString();
                txtLeft.Text = _screen.PhysicalBounds.Left.ToString();
                txtRight.Text = _screen.PhysicalBounds.Right.ToString();

                lblDPI.Text = _screen.DpiAvg.ToString();

                lblName.Content = _screen.DeviceName;
        }

         public ScreenGUI(Screen s)
        {
            InitializeComponent();
            Screen = s;
        }

        private void lblDPI_TextChanged(object sender, TextChangedEventArgs e)
        {
           // _screen.DpiX = double.Parse(lblDPI.Text);
           // _screen.DpiY = double.Parse(lblDPI.Text);
        }
    }
}
