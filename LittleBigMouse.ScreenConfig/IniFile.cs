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

using System.Runtime.InteropServices;
using System.Text;

namespace LittleBigMouse.DisplayLayout;

/// <summary>
/// Create a New INI file to store or load data
/// </summary>
public class IniFile
{
    public string Path;

    [DllImport("kernel32")]
    static extern long WritePrivateProfileString(string section,
        string key, string val, string filePath);
    [DllImport("kernel32")]
    static extern int GetPrivateProfileString(string section,
        string key, string def, StringBuilder retVal,
        int size, string filePath);

    /// <summary>
    /// INIFile Constructor.
    /// </summary>
    /// <PARAM name="iniPath"></PARAM>
    public IniFile(string iniPath)
    {
        Path = iniPath;
    }
    /// <summary>
    /// Write Data to the INI File
    /// </summary>
    /// <PARAM name="section"></PARAM>
    /// section name
    /// <PARAM name="key"></PARAM>
    /// key Name
    /// <PARAM name="value"></PARAM>
    /// value Name
    public void WriteValue(string section, string key, string value)
    {
        WritePrivateProfileString(section, key, value, Path);
    }

    /// <summary>
    /// Read Data value From the Ini File
    /// </summary>
    /// <PARAM name="section"></PARAM>
    /// <PARAM name="key"></PARAM>
    /// <PARAM name="Path"></PARAM>
    /// <returns></returns>
    public string ReadValue(string section, string key, string def)
    {
        StringBuilder temp = new StringBuilder(255);
        int i = GetPrivateProfileString(section, key, def, temp,
            255, Path);
        return temp.ToString();

    }
}