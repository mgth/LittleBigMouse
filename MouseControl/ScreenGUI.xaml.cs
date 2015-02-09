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
        private double _ratio;

        private Screen _screen;
        public Screen Screen {
            get { return _screen; }
            set { _screen = value;
                txtTop.Text = value.PhysicalBounds.Top.ToString();
                txtBottom.Text = value.PhysicalBounds.Bottom.ToString();
                txtLeft.Text = value.PhysicalBounds.Left.ToString();
                txtRight.Text = value.PhysicalBounds.Right.ToString();

                lblDPI.Text = value.DpiAvg.ToString();

                lblName.Content = value.DeviceName;

                _screen.PhysicalSizeChanged += _screen_PhysicalSizeChanged;
            }
        }


        public double Ratio
        {
            get  { return _ratio;  }
            set
            {
                _ratio = value;
                setRenderSize();
            }
        }

        private void setRenderSize()
        {
            this.RenderSize = new Size(_screen.PhysicalBounds.Width * _ratio, _screen.PhysicalBounds.Height * _ratio);
        }

        private void _screen_PhysicalSizeChanged(object sender, EventArgs e)
        {
            setRenderSize();
        }

        public ScreenGUI()
        {
            InitializeComponent();
        }

        private void lblDPI_TextChanged(object sender, TextChangedEventArgs e)
        {
           // _screen.DpiX = double.Parse(lblDPI.Text);
           // _screen.DpiY = double.Parse(lblDPI.Text);
        }
    }
}
