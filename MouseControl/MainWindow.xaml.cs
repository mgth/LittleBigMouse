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
        private Notify _notify = new Notify();
        public MainWindow()
        {
            _MouseHookManager.MouseMoveExt += _MouseHookManager_MouseMoveExt;
            _MouseHookManager.Enabled = true;
            InitializeComponent();

            _notify.Click += _notify_Click;

            this.ShowInTaskbar = false;

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
            /*
                        Zones.Add(new Zone(Screen.getScreen(3), Screen.getScreen(3).LeftRect, Screen.getScreen(2).Bounds, Math.Round(10.0 * 102.0 /185.0,0),1));
                        Zones.Add(new Zone(Screen.getScreen(2), Screen.getScreen(2).RightRect, Screen.getScreen(3).Bounds, 10.0,3));

                        Zones.Add(new Zone(Screen.getScreen(3), Screen.getScreen(3).RightRect, Screen.getScreen(1).Bounds,Math.Round(10.0 * 96.0/185.0,0),1));
                        Zones.Add(new Zone(Screen.getScreen(1), Screen.getScreen(1).LeftRect, Screen.getScreen(3).Bounds, 10.0,3));
            */
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


        private Point _oldPoint; 

        private void _MouseHookManager_MouseMoveExt(object sender, MouseEventExtArgs e)
        {
            // TODO : remove
            labelX.Content = e.X;
            labelY.Content = e.Y;

            Point pIn = new Point(e.X, e.Y);

            if (Screen == null ) Screen = Screen.FromPoint(pIn);

            if (Screen.Bounds.Contains(pIn))
            {
                _oldPoint = pIn;
                return;
            }

            Point pOutPhysical = Screen.PixelToPhysical(pIn);

            Screen screenOut = Screen.FromPhysicalPoint(pOutPhysical);

            if (screenOut!=null)
            {
                Point pOut = screenOut.PhysicalToPixel(pOutPhysical);

                Mouse.SetCursorPos((int)pOut.X, (int)pOut.Y);

                Screen = Screen.FromPoint(pOut);

                if (Screen.DPI > 110)
                {
                    if (Screen.DPI > 138)
                        Mouse.setCursorAero(3);
                    else Mouse.setCursorAero(2);
                } else Mouse.setCursorAero(1);

                Mouse.MouseSpeed = Math.Round((5.0 / 96.0) * Screen.DPI,0);

                _oldPoint = pIn;
            }
            else
            {
                Mouse.SetCursorPos((int)_oldPoint.X,(int)_oldPoint.Y);
            }

            e.Handled = true;
            /*
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
                        }*/
        }

        private void button_Click(object sender, RoutedEventArgs e)
        {
            Config cfg = new Config();
            cfg.ShowDialog();
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