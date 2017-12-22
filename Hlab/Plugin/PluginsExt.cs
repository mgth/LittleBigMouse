/*
  HLab.Plugin
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Plugin.

    HLab.Plugin is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Plugin is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.Collections;
using System.Globalization;
using System.Resources;
using System.Windows;

namespace HLab.Plugin
{
    public static class PluginsExt
    {
        public static void RegisterIcons(this IPluginModule module)
        {
            var assembly = module.GetType().Assembly;
            var rm = new ResourceManager(assembly.GetName().Name + ".g", assembly);
            try
            {
                var dict = new ResourceDictionary();
                var list = rm.GetResourceSet(CultureInfo.CurrentCulture, true, true);
                foreach (DictionaryEntry item in list)
                {
                    var s = item.Key.ToString();
                    if (!s.StartsWith("icons/")) continue;

                    var uri = new Uri("/" + assembly.GetName().Name + ";component/" + s.Replace(".baml", ".xaml"), UriKind.RelativeOrAbsolute);
                    var obj = Application.LoadComponent(uri);

                    var key = s.Replace("icons/", "").Replace(".baml", "");

                    if (obj.GetType() == typeof(ResourceDictionary))
                    {
                        Application.Current.Resources.MergedDictionaries.Add(obj as ResourceDictionary);
                    }
                    else
                        dict.Add(key, obj);
                }
                Application.Current.Resources.MergedDictionaries.Add(dict);
            }
            finally
            {
                rm.ReleaseAllResources();
            }
        }
    }
}