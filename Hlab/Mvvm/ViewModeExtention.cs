/*
  HLab.Mvvm
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Mvvm.

    HLab.Mvvm is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Mvvm is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.Windows.Markup;

namespace Hlab.Mvvm
{


        [MarkupExtensionReturnType(typeof(Type))]
    public class ViewModeExtention : MarkupExtension
    {
        public string ViewModeName { get; set; }
        public ViewModeExtention(string name)
        {
            ViewModeName = name;
        }
        public override object ProvideValue(IServiceProvider serviceProvider)
        {
            if (string.IsNullOrEmpty(ViewModeName))
            {
                throw new ArgumentException("The variable name can't be null or empty");
            }

            return Type.GetType(ViewModeName);
        }
    }
}
