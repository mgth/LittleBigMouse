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
            set { _screen = value;
                txtTop.Text = value.Bounds.Top.ToString();
                txtBottom.Text = value.Bounds.Bottom.ToString();
                txtLeft.Text = value.Bounds.Left.ToString();
                txtRight.Text = value.Bounds.Right.ToString();

                lblName.Content = value.DeviceName;

            }
        }
        public ScreenGUI()
        {
            InitializeComponent();
        }
    }
}
