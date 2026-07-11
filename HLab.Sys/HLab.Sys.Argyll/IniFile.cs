/*
  HLab.Argyll
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Argyll.

    HLab.Argyll is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Argyll is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;

namespace HLab.Sys.Argyll;

/// <summary>
/// Read or store data in an INI file. Managed implementation with
/// GetPrivateProfileString-like semantics (the previous kernel32 P/Invoke threw
/// DllNotFoundException on Linux): case-insensitive section and key lookup,
/// trimmed values, surrounding quotes stripped, default when missing.
/// </summary>
public class IniFile
{
    public string Path;

    public IniFile(string iniPath)
    {
        Path = iniPath;
    }

    public string ReadValue(string section, string key, string def)
    {
        try
        {
            if (!File.Exists(Path)) return def;

            var inSection = false;
            foreach (var raw in File.ReadLines(Path))
            {
                var line = raw.Trim();
                if (line.Length == 0 || line[0] == ';' || line[0] == '#') continue;

                if (line[0] == '[' && line[^1] == ']')
                {
                    inSection = line[1..^1].Trim().Equals(section, StringComparison.OrdinalIgnoreCase);
                    continue;
                }

                if (!inSection) continue;

                var eq = line.IndexOf('=');
                if (eq < 0) continue;
                if (!line[..eq].Trim().Equals(key, StringComparison.OrdinalIgnoreCase)) continue;

                var value = line[(eq + 1)..].Trim();
                if (value.Length >= 2 &&
                    ((value[0] == '"' && value[^1] == '"') || (value[0] == '\'' && value[^1] == '\'')))
                    value = value[1..^1];
                return value;
            }
        }
        catch (IOException)
        {
        }

        return def;
    }

    public void WriteValue(string section, string key, string value)
    {
        var lines = File.Exists(Path) ? File.ReadAllLines(Path).ToList() : new List<string>();

        var sectionStart = -1;
        var sectionEnd = lines.Count;

        for (var i = 0; i < lines.Count; i++)
        {
            var line = lines[i].Trim();
            if (line.Length == 0 || line[0] != '[' || line[^1] != ']') continue;

            if (sectionStart >= 0) { sectionEnd = i; break; }

            if (line[1..^1].Trim().Equals(section, StringComparison.OrdinalIgnoreCase))
                sectionStart = i;
        }

        if (sectionStart < 0)
        {
            lines.Add($"[{section}]");
            lines.Add($"{key} = {value}");
        }
        else
        {
            var keyLine = -1;
            for (var i = sectionStart + 1; i < sectionEnd; i++)
            {
                var line = lines[i].Trim();
                var eq = line.IndexOf('=');
                if (eq < 0) continue;
                if (!line[..eq].Trim().Equals(key, StringComparison.OrdinalIgnoreCase)) continue;
                keyLine = i;
                break;
            }

            if (keyLine >= 0) lines[keyLine] = $"{key} = {value}";
            else lines.Insert(sectionEnd, $"{key} = {value}");
        }

        File.WriteAllLines(Path, lines);
    }
}
