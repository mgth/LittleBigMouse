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

// Registry helpers for the Windows persistence layer.
namespace LittleBigMouse.Platform.Windows;

#pragma warning disable CA1416


public static class RegistryExt
{
    public static string GetOrSet(this RegistryKey key, string keyName, Func<string> def, Action ifLoaded = null)
    {
        (key, keyName) = key.ParseKeyName(keyName);

        var v = key.GetValue(keyName);
        if (v is string r)
        {
            ifLoaded?.Invoke();
        }
        else
        {
            r = def.Invoke();
            key.SetValue(keyName, r,RegistryValueKind.String);
        }

        return r;
    }

    public static bool GetOrSet(this RegistryKey key, string keyName, Func<bool> getter, Action ifLoaded = null)
        => GetOrSet(key, keyName, () => getter() ? "1" : "0", ifLoaded) == "1";

    public static double GetOrSet(this RegistryKey key, string keyName, Func<double> getter, Action ifLoaded = null)
        => double.Parse(
            GetOrSet(key, keyName, () => getter().ToString(CultureInfo.InvariantCulture), ifLoaded ),
            CultureInfo.InvariantCulture);

    public static int GetOrSet(this RegistryKey key, string keyName, Func<int> getter, Action ifLoaded = null)
        => int.Parse(
            GetOrSet(key, keyName, () => getter().ToString(CultureInfo.InvariantCulture), ifLoaded ),
            CultureInfo.InvariantCulture);

    /// <summary>
    /// Read a stored double without seeding the store: unlike GetOrSet, an absent
    /// value stays absent (used to detect whether a monitor owns per-monitor data).
    /// </summary>
    public static double? TryGet(this RegistryKey key, string keyName)
        => key.TryGetString(keyName) is string s
           && double.TryParse(s, NumberStyles.Float, CultureInfo.InvariantCulture, out var v)
            ? v
            : null;

    /// <summary>Read a stored string without seeding the store; null when absent.</summary>
    public static string TryGetString(this RegistryKey key, string keyName)
    {
        var i = keyName.LastIndexOf('\\');
        if (i >= 0)
        {
            using var sub = key.OpenSubKey(keyName[..i]);
            return sub?.TryGetString(keyName[(i + 1)..]);
        }

        return key.GetValue(keyName) as string;
    }

    /// <summary>Read a stored bool ("1"/"0") without seeding the store; null when absent.</summary>
    public static bool? TryGetBool(this RegistryKey key, string keyName)
        => key.TryGetString(keyName) is string s ? s == "1" : null;

    /// <summary>Read a stored int without seeding the store; null when absent.</summary>
    public static int? TryGetInt(this RegistryKey key, string keyName)
        => key.TryGetString(keyName) is string s
           && int.TryParse(s, NumberStyles.Integer, CultureInfo.InvariantCulture, out var v)
            ? v
            : null;

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

    public static void SetKey(this RegistryKey key, string keyName, int value)
    {
        (key, keyName) = key.ParseKeyName(keyName);
        key.SetValue(keyName, value.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String);
    }
}
