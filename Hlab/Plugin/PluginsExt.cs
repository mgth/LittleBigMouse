using System;
using System.Collections;
using System.Globalization;
using System.Resources;
using System.Windows;

namespace Hlab.Plugin
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