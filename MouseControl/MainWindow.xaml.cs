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
        private readonly MouseHookListener _MouseHookManager = new MouseHookListener(new GlobalHooker());

        private double _mouseSpeed = Mouse.MouseSpeed;
        public MainWindow()
        {
            _MouseHookManager.MouseMoveExt += _MouseHookManager_MouseMoveExt;
            _MouseHookManager.Enabled = true;
            InitializeComponent();

                text.Text = "";
            foreach (Screen s in Screen.AllScreens)
                text.Text += s.DeviceName + " : " + s.Bounds.Left + "-" + s.Bounds.Right + " , " + s.Bounds.Top + "-" + s.Bounds.Bottom + " : " + s.Bounds.Width + "-" + s.Bounds.Height + "\n";

            // c'est la qu'il faut creer les zones : 
            // - L'ecran d'où on vient
            // - Le rectangle dans lequelle on rentre
            // - Le rectangle vers lequelle deplacer la souris
            // - La vitesse de la souris
            // - La taille de pointeur : 1 2 ou 3
            // tu peux coller un rectangle, mais j'ai fais des fonctions dans Screen qui renvoient 
            // toute la zone à coté 

            Zones.Add(new Zone(Screen.GetScreen(3), Screen.GetScreen(3).LeftRect, Screen.GetScreen(2).Bounds, Math.Round(10.0 * 102.0 /185.0,0),1));
            Zones.Add(new Zone(Screen.GetScreen(2), Screen.GetScreen(2).RightRect, Screen.GetScreen(3).Bounds, 10.0,3));

            Zones.Add(new Zone(Screen.GetScreen(3), Screen.GetScreen(3).RightRect, Screen.GetScreen(1).Bounds,Math.Round(10.0 * 96.0/185.0,0),1));
            Zones.Add(new Zone(Screen.GetScreen(1), Screen.GetScreen(1).LeftRect, Screen.GetScreen(3).Bounds, 10.0,3));
        }


        public List<Zone> Zones = new List<Zone>();

        private Screen _screen = null;

        public Screen Screen
        {
            get { return _screen; }
            set
            {
                _screen = value;
                labelScreenName.Content = _screen.DeviceName;
            }
        }

        private Screen getScreen(Point p)
        {
            Screen s = Screen.FromPoint(p);
            if (s.Bounds.Contains(p)) return s; 
            
            return null;
        }




        private void _MouseHookManager_MouseMoveExt(object sender, MouseEventExtArgs e)
        {
            // TODO : remove
            labelX.Content = e.X;
            labelY.Content = e.Y;

            Point pIn = new Point(e.X, e.Y);

            if (Screen == null ) Screen = Screen.FromPoint(pIn);

            if (Screen.Bounds.Contains(pIn)) return;

            foreach(Zone z in Zones)
            {
                if (z.Screen.DeviceName==Screen.DeviceName && z.Contains(pIn))
                {
                    Point pOut = z.Translate(pIn);
                    Screen = Screen.FromPoint(pOut);
                    //System.Windows.Forms.Cursor.Position = new System.Drawing.Point((int)Math.Round(pOut.X,0), (int)Math.Round(pOut.Y, 0));
                    Mouse.SetCursorPos((int)Math.Round(pOut.X, 0), (int)Math.Round(pOut.Y, 0));
                    Mouse.MouseSpeed = z.Speed;
                    Mouse.setCursorAero(z.Size);
                    //labelSpeed.Content = z.DpiRatio.ToString();
                    labelSpeed.Content = Mouse.MouseSpeed.ToString();
                    e.Handled = true;
                    return;
                }
            }
        }

        private void button_Click(object sender, RoutedEventArgs e)
        {
            Config cfg = new Config();
            cfg.ShowDialog();
        }

        private void Window_Unloaded(object sender, RoutedEventArgs e)
        {
            Mouse.MouseSpeed = _mouseSpeed;
        }

        private void Window_Closed(object sender, EventArgs e)
        {
            Mouse.MouseSpeed = _mouseSpeed;
        }
    }
}