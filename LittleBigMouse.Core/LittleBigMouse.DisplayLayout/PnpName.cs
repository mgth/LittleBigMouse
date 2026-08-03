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

using System.Text.RegularExpressions;

namespace LittleBigMouse.DisplayLayout;

/// <summary>
/// Tidies the monitor name Windows reports, which tends to arrive as
/// "Dell U2415 Drivers (DP)".
/// </summary>
/// <remarks>
/// This used to sit next to a set of scrapers that looked a PnP code up on
/// driverlookup.com, driveragent.com and driversdownloader.com. None of those
/// endpoints answer any more, nothing had called them in years, and an HTTP
/// scraper aimed at a third-party site is not something to leave lying around in
/// an application that otherwise only talks to GitHub and to televisions on the
/// local network. Only the local string cleanup was ever reached.
/// </remarks>
public static class PnpName
{
    public static string Cleanup(string result)
    {
        if (result.Contains("Drivers")) result = result.Replace("Drivers", "");

        var match2 = Regex.Match(result, @"\((.*?)\)", RegexOptions.Singleline);

        for (var i = 1; i < match2.Groups.Count; i++)
        {
            result = result.Replace("(" + match2.Groups[i].Value + ")", "");
        }

        return result.Trim();
    }
}
