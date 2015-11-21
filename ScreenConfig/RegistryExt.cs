using System.Globalization;
using Microsoft.Win32;

namespace LbmScreenConfig
{
    public static class RegistryExt
    {
        public static void GetKey(this RegistryKey key, ref double prop, string keyName, PropertyChangeHandler change=null)
        {
            string sValue = key.GetValue(keyName, "NaN").ToString();
            if (sValue == "NaN") return;

            double value = double.Parse(sValue, CultureInfo.InvariantCulture);
            if (change ==null) prop = value;
            else
                change.SetProperty(ref prop, value, keyName); 
        }

        public static void SetKey(this RegistryKey key, double prop, string keyName)
        {
            if (double.IsNaN(prop)) { key.DeleteValue(keyName, false); }
            else { key.SetValue(keyName, prop.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String); }
        }

        public static void GetKey(this RegistryKey key,ref string prop,  string keyName, PropertyChangeHandler change = null)
        {
            string sValue = (string)key.GetValue(keyName, null);
            if (sValue == null) return;

            if (change == null) prop = sValue;
            else change?.SetProperty(ref prop, sValue, keyName);
        }

        public static void SetKey(this RegistryKey key, string prop, string keyName)
        {
            if (prop == null) { key.DeleteValue(keyName, false); }
            else { key.SetValue(keyName, prop.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String); }
        }
        public static void GetKey(this RegistryKey key, ref bool prop, string keyName, PropertyChangeHandler change = null)
        {
            string sValue = (string)key.GetValue(keyName, "0");
            if (sValue == null) return;

            if (change == null) prop = sValue=="1";
            else change?.SetProperty(ref prop, sValue == "1", keyName);
        }

        public static void SetKey(this RegistryKey key, ref bool prop, string keyName)
        {
            key.SetValue(keyName, prop?"1":"0", RegistryValueKind.String); 
        }
    }
}
