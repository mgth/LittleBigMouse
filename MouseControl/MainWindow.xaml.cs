using MouseKeyboardActivityMonitor;
using MouseKeyboardActivityMonitor.WinApi;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
//using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Forms;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
//using System.Windows.Shapes;

namespace MouseControl
{
    /// <summary>
    /// Interaction logic for MainWindow.xaml
    /// </summary>
    public partial class MainWindow : Window
    {
        private ScreenConfig _config;

        private double _mouseSpeed = Mouse.MouseSpeed;
        private Notify _notify = new Notify();
        public MainWindow()
        {
            InitializeComponent();

            LoadConfig();

            _notify.Click += _notify_Click;

            this.ShowInTaskbar = false;

            text.Text = "";
            foreach (Screen s in _config.AllScreens)
                text.Text += s.DeviceName + " : " + s.Bounds.Left + "-" + s.Bounds.Right + " , " + s.Bounds.Top + "-" + s.Bounds.Bottom + " : " + s.Bounds.Width + "-" + s.Bounds.Height + "\n";
        }

        private void _notify_Click(object sender, EventArgs e)
        {
            if (this.Visibility==Visibility.Hidden)
            {
                this.Visibility = Visibility.Visible;
                this.Activate();
            }
            else    this.Hide();          
        }


        private void button_Click(object sender, RoutedEventArgs e)
        {
            ScreenConfig scr = ScreenConfig.Load();

            scr.RegistryChanged += Scr_RegistryChanged;

            Config cfg = new Config(scr);
            cfg.ShowDialog();
        }

        private void LoadConfig()
        {
            if (_config != null) _config.Disable();
            _config=ScreenConfig.Load();
            _config.Enable();
        }

        private void Scr_RegistryChanged(object sender, EventArgs e)
        {
            LoadConfig();
        }

        private void Window_Unloaded(object sender, RoutedEventArgs e)
        {

        }

        private void Window_Closed(object sender, EventArgs e)
        {
            Mouse.MouseSpeed = 10.0;
            Mouse.setCursorAero(1);
        }

        private void Window_StateChanged(object sender, EventArgs e)
        {

        }
    }
}