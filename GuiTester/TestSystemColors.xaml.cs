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

namespace LittleBigMouse.Control.Main
{
    /// <summary>
    /// Logique d'interaction pour TestSystemColors.xaml
    /// </summary>
    public partial class TestSystemColors : UserControl
    {
        public TestSystemColors()
        {
            InitializeComponent();

            var sys = typeof(SystemColors);

            var prop = sys.GetProperties(System.Reflection.BindingFlags.Static|System.Reflection.BindingFlags.Public);

            foreach (var item in prop)
            {
                if(item.PropertyType == typeof(SolidColorBrush))
                {
                    if(item.GetValue(null) is SolidColorBrush brush)
                    {
                        var tb = new TextBox() { 
                            Foreground = Brushes.Black,
                            Text = item.Name, 
                            Background = brush,
                            };
                        StackPanel.Children.Add(tb);

                        tb = new TextBox() {
                            Foreground = Brushes.White,
                            Text = item.Name, 
                            Background = brush,
                            };
                        StackPanel.Children.Add(tb);
                    }
                }
            }
        }
    }
}
