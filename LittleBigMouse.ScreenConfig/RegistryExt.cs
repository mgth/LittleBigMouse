/*
  LittleBigMouse.Screen.Config
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Screen.Config.

    LittleBigMouse.Screen.Config is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Screen.Config is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Globalization;

using Microsoft.Win32;

namespace LittleBigMouse.DisplayLayout
{
    public static class RegistryExt
    {
        public static T GetKey<T>(this RegistryKey key, string keyName, Func<T> def = null, Action ifLoaded = null)
        {
            (key, keyName) = key.ParseKeyName(keyName);

            def ??= () => default;

            if (key == null) return def();

            object value = null;

            var sValue = key.GetValue(keyName, "")?.ToString() ?? "";
            if (sValue == "") return def();

            if (typeof(T) == typeof(double))
            {
                value = double.Parse(sValue, CultureInfo.InvariantCulture);
            }
            else if (typeof(T) == typeof(string))
            {
                value = sValue;
            }
            else if (typeof(T) == typeof(string))
            {
                value = sValue == "1";
            }

            ifLoaded?.Invoke();

            return (T)value;
        }

        static (RegistryKey, string) ParseKeyName(this RegistryKey key, string keyName)
        {
            var i = keyName.LastIndexOf('\\');
            if (i < 0) return (key, keyName);
            var subKey = keyName[..i];
            keyName = keyName[(i + 1)..];
            var sub = key.OpenSubKey(subKey, true) ?? key.CreateSubKey(subKey);
            return (sub, keyName);
        }

        public static void SetKey(this RegistryKey key, string keyName, double value)
        {
            (key, keyName) = key.ParseKeyName(keyName);
            if (double.IsNaN(value)) { key.DeleteValue(keyName, false); }
            else { key.SetValue(keyName, value.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String); }
        }


        public static void SetKey(this RegistryKey key, string keyName, string value)
        {
            (key, keyName) = key.ParseKeyName(keyName);
            if (value == null) { key.DeleteValue(keyName, false); }
            else { key.SetValue(keyName, value.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String); }
        }

        public static void SetKey(this RegistryKey key, string keyName, bool value)
        {
            (key, keyName) = key.ParseKeyName(keyName);
            key.SetValue(keyName, value ? "1" : "0", RegistryValueKind.String);
        }
    }
}
