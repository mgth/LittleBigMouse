using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using LbmScreenConfig;

namespace LittleBigMouse_Control
{
    /// <summary>
    /// Logique d'interaction pour TestPatternWindow.xaml
    /// </summary>
    public partial class TestPatternWindow : Window
    {
        public TestPatternType PatternType
        {
            set { pattern.PatternType = value; }
            get { return pattern.PatternType; }
        }
        public Color PatternColor
        {
            set { pattern.PatternColor = value; }
            get { return pattern.PatternColor; }
        }
        public TestPatternWindow()
        {
            InitializeComponent();
        }
        private void Window_PreviewKeyDown(object sender, KeyEventArgs e)
        {
            Window win = (Window)sender;

            if (e.Key == Key.Escape)
                win.Visibility = System.Windows.Visibility.Hidden;//.Close();
                                                                  //else if( e.Key == Key.Left )

        }

        public void ShowOnScreen(Screen s)
        {
            if (s != null)
            {
                Left = s.Bounds.TopLeft.Wpf.X;
                Top = s.Bounds.TopLeft.Wpf.Y;
                Width = s.Bounds.BottomRight.Wpf.X - s.Bounds.TopLeft.Wpf.X;
                Height = s.Bounds.BottomRight.Wpf.Y - s.Bounds.TopLeft.Wpf.Y;

                Show();
            }
            else
            {
                Hide();
            }
        }

    }
}