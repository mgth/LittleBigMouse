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
        }

        private void _screen_PhysicalChanged(object sender, EventArgs e)
        {
            UpdateValues();
        }

        public void UpdateValues()
        {
                txtTop.Content = _screen.PhysicalBounds.Top.ToString("0.0");
                txtBottom.Content = _screen.PhysicalBounds.Bottom.ToString("0.0");
                txtLeft.Content = _screen.PhysicalBounds.Left.ToString("0.0");
                txtRight.Content = _screen.PhysicalBounds.Right.ToString("0.0");

                lblDPI.Content = _screen.DpiAvg.ToString("0");

                lblName.Content = _screen.DeviceNo.ToString();

                Rect r = Screen.ToUI(new Size(_grid.ActualWidth, _grid.ActualHeight));

                Margin = new Thickness(
                    r.X,
                    r.Y,
                    0, 0);
        }

        private Grid _grid;
         public ScreenGUI(Screen s, Grid grid)
        {
            _grid = grid;
            _screen = s;
            InitializeComponent();

            _screen.PhysicalChanged += _screen_PhysicalChanged;
            UpdateValues();
        }

        private void lblDPI_TextChanged(object sender, TextChangedEventArgs e)
        {
           // _screen.DpiX = double.Parse(lblDPI.Text);
           // _screen.DpiY = double.Parse(lblDPI.Text);
        }

        private List<Sizer> _sizers = new List<Sizer>();

        private void AddSizer(SizerSide side)
        {
            Sizer sz = Sizer.getSizer(_screen, side);

            if (sz!=null)
            {
                _sizers.Add(sz);
                sz.Show();
            }
        }

        public void ShowSizers()
        {
            HideSizers();

            AddSizer(SizerSide.Left);
            AddSizer(SizerSide.Right);
            AddSizer(SizerSide.Top);
            AddSizer(SizerSide.Bottom);
        }

        public void HideSizers()
        {
                foreach(Sizer sz in _sizers)
                {
                    sz.Close();
                }
                _sizers.Clear();
        }

        public void SwitchSizers()
        {
            if (_sizers.Count()>0)
            {
                HideSizers();
            }
            else
            {
                ShowSizers();
            }
        }

        private void cmdSize_Click(object sender, RoutedEventArgs e)
        {
            SwitchSizers();
        }

        private void Grid_SizeChanged(object sender, SizeChangedEventArgs e)
        {
            center.Height = Math.Min(grid.ActualHeight, grid.ActualWidth)/3;
            center.Width =  center.Height;
            center.CornerRadius = new CornerRadius(center.Height / 2);

            lblName.FontSize = center.Height / 2;
        }
    }
}
