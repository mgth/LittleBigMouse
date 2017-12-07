using System.Collections.Generic;
using System.Reflection;
using System.Windows;
using System.Windows.Media;

namespace LittleBigMouse.Control.Core
{
    public partial class ColorsWindow : Window
    {
        public ColorsWindow()
        {
            InitializeComponent();

            List<ColorAndName> l = new List<ColorAndName>();

            foreach (PropertyInfo i in typeof(System.Windows.SystemColors).GetProperties())
            {
                if (i.PropertyType == typeof(Color))
                {
                    ColorAndName cn = new ColorAndName();
                    cn.Color = (Color)i.GetValue(new Color(), BindingFlags.GetProperty, null, null, null);
                    cn.Name = i.Name;
                    l.Add(cn);
                }
            }

            SystemColorsList.DataContext = l;
        }
    }

    class ColorAndName
    {
        public Color Color { get; set; }
        public string Name { get; set; }
    }
}