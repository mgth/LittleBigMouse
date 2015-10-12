using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Shapes;
using WinAPI_User32;

namespace LittleBigMouse
{
    /// <summary>
    /// Logique d'interaction pour InsideRuler.xaml
    /// </summary>
    public partial class InsideRuler : Window
    {
        private Screen _screen;
        public InsideRuler(Screen screen)
        {
            _screen = screen;
            InitializeComponent();

            Rect r = new Rect(screen.Bounds.TopLeft.DpiAware.Point,screen.Bounds.BottomRight.DpiAware.Point);

            Left = r.Left;
            Top = r.Top;
            Width = r.Width;
            Height = r.Height;

            double ratioX = screen.RatioX;
            double ratioY = screen.RatioY;

            int i = 0;
            while (true)
            {
                double x = i * ratioX;
                if (x > Width) break;

                double y; string text=null;
                if (i % 100 == 0)
                {
                    y = 20.0 * ratioY;
                    if (i > 0) text = (i / 100).ToString();
                }
                else if (i % 50 == 0)
                {
                    y = 15.0 * ratioY;
                    text = "5";
                }
                else if (i % 10 == 0)
                {
                    y = 10.0 * ratioY;
                    text = ((i%100) / 10).ToString();
                }
                else if (i % 5 == 0)
                {
                    y = 5.0 * ratioY;
                }
                else
                {
                    y = 2.5 * ratioY;
                }

                if (text!=null)
                {
                    TextBlock t = new TextBlock
                    {
                        Text = text,
                        FontSize = y / 3,
                    };
                    t.SetValue(Canvas.LeftProperty, x);
                    t.SetValue(Canvas.TopProperty, y - t.FontSize);
                    canvas.Children.Add(t);

                    TextBlock t2 = new TextBlock
                    {
                        Text = text,
                        FontSize = y / 3,
                    };
                    t2.SetValue(Canvas.LeftProperty, x);
                    t2.SetValue(Canvas.TopProperty, Height - y);
                    canvas.Children.Add(t2);
                }

                Line l = new Line
                {
                    X1 = x,
                    X2 = x,
                    Y1 = 0,
                    Y2 = y,
                    Stroke = new SolidColorBrush(Colors.Black),
                };
                canvas.Children.Add(l);

                Line l2 = new Line
                {
                    X1 = x,
                    X2 = x,
                    Y1 = Height - y,
                    Y2 = Height,
                    Stroke = new SolidColorBrush(Colors.Black),
                };
                canvas.Children.Add(l2);
                i++;
            }

            i = 0;
            while (true)
            {
                double y = i * ratioY;
                if (y > Height) break;

                double x; string text = null;
                if (i % 100 == 0)
                {
                    x = 20.0 * ratioX;
                    if (i>0) text = (i / 100).ToString();
                }
                else if (i % 50 == 0)
                {
                    x = 15.0 * ratioX;
                    text = "5";
                }
                else if (i % 10 == 0)
                {
                    x = 10.0 * ratioX;
                    text = ((i % 100) / 10).ToString();
                }
                else if (i % 5 == 0)
                {
                    x = 5.0 * ratioX;
                }
                else
                {
                    x = 2.5 * ratioX;
                }

                if (text != null)
                {
                    TextBlock t = new TextBlock
                    {
                        Text = text,
                        FontSize = x / 3,
                    };
                    t.SetValue(Canvas.LeftProperty, x - t.FontSize);
                    t.SetValue(Canvas.TopProperty, y);
                    canvas.Children.Add(t);

                    TextBlock t2 = new TextBlock
                    {
                        Text = text,
                        FontSize = x / 3,
                    };
                    t2.SetValue(Canvas.LeftProperty, Width-x);
                    t2.SetValue(Canvas.TopProperty, y);
                    canvas.Children.Add(t2);
                }

                Line l = new Line
                {
                    X1 = 0,
                    X2 = x,
                    Y1 = y,
                    Y2 = y,
                    Stroke = new SolidColorBrush(Colors.Black),
                };
                canvas.Children.Add(l);

                Line l2 = new Line
                {
                    X1 = Width-x,
                    X2 = Width,
                    Y1 = y,
                    Y2 = y,
                    Stroke = new SolidColorBrush(Colors.Black),
                };
                canvas.Children.Add(l2);
                i++;
            }
        }

        public const int WS_EX_TRANSPARENT = 0x00000020;
        public const int GWL_EXSTYLE = (-20);


        protected override void OnSourceInitialized(EventArgs e)
        {
            base.OnSourceInitialized(e);

            // Get this window's handle
            IntPtr hwnd = new WindowInteropHelper(this).Handle;

            // Change the extended window style to include WS_EX_TRANSPARENT
            int extendedStyle = User32.GetWindowLong(hwnd, GWL_EXSTYLE);
            User32.SetWindowLong(hwnd, GWL_EXSTYLE, extendedStyle | WS_EX_TRANSPARENT);
        }
    }

}
