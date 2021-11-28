/*
  LittleBigMouse.Control.Core
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Control.Core.

    LittleBigMouse.Control.Core is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Control.Core is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System.Collections.Generic;
using System.Reflection;
using System.Windows;
using System.Windows.Media;

namespace LittleBigMouse.Control
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