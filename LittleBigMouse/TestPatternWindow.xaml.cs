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
using System.Windows.Shapes;

namespace LittleBigMouse
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
    }
}
