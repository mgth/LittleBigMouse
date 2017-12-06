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
            set => pattern.PatternType = value; get => pattern.PatternType;
        }
        public Color PatternColor
        {
            set => pattern.PatternColor = value; get => pattern.PatternColor;
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


        //TODO
        public void ShowOnScreen(Screen s)
        {
            if (s != null)
            {
                //Left = s.Bounds.TopLeft.Dip.X;
                //Top = s.Bounds.TopLeft.Dip.Y;
                //Width = s.Bounds.BottomRight.Dip.X - s.Bounds.TopLeft.Dip.X;
                //Height = s.Bounds.BottomRight.Dip.Y - s.Bounds.TopLeft.Dip.Y;

                Show();
            }
            else
            {
                Hide();
            }
        }

    }
}