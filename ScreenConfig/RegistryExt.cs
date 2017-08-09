using System;
using System.Globalization;
using Microsoft.Win32;

namespace LbmScreenConfig
{
    public static class RegistryExt
    {
        public static T GetKey<T>(this RegistryKey key, string keyName, Func<T> def = null)
        {
            if (def == null) def = () => default(T);

            if (key == null) return def();

            object value = null;

            string sValue = key.GetValue(keyName, "").ToString();
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

            return (T) value;
        }

        public static void SetKey(this RegistryKey key, string keyName, double value)
        {
            if (double.IsNaN(value)) { key.DeleteValue(keyName, false); }
            else { key.SetValue(keyName, value.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String); }
        }


        public static void SetKey(this RegistryKey key, string keyName, string value)
        {
            if (value == null) { key.DeleteValue(keyName, false); }
            else { key.SetValue(keyName, value.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String); }
        }

        public static void SetKey(this RegistryKey key, string keyName, bool value)
        {
            key.SetValue(keyName, value ? "1":"0", RegistryValueKind.String); 
        }
    }
}
