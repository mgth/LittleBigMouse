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
    public partial class ScreenProperties : UserControl, IPropertyPane
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private void changed(String name)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
        }

        private Screen _screen = null;
        public Screen Screen
        {
            get { return _screen; }
            set {
                if (cmdSizers.IsChecked??false)
                {
                    HideSizers();
                }

                _screen = value;

                if (_screen != null && (cmdSizers.IsChecked??false))
                {
                    ShowSizers();
                }
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

        private List<Sizer> _sizers = new List<Sizer>();
        private void AddSizer(SizerSide side)
        {
            if (Screen == null) return;

            foreach (var sz in Screen.Config.AllScreens.Select(s => new Sizer(Screen, s, side)))
            {
                _sizers.Add(sz);
            }
        }

        public void ShowSizers()
        {
            HideSizers();

            AddSizer(SizerSide.Left);
            AddSizer(SizerSide.Right);
            AddSizer(SizerSide.Top);
            AddSizer(SizerSide.Bottom);

            foreach (Sizer sz in _sizers) sz.Enabled = true;
        }
        public void HideSizers()
        {
                foreach(Sizer sz in _sizers)
                {
                    sz.Close();
                }
                _sizers.Clear();
        }

        private void cmdSizers_Checked(object sender, RoutedEventArgs e)
        {
            ShowSizers();
        }

        private void cmdSizers_Unchecked(object sender, RoutedEventArgs e)
        {
            HideSizers();
        }
    }
}
