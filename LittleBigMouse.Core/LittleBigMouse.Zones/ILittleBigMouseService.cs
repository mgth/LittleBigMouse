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

namespace LittleBigMouse.Zoning;

public enum LittleBigMouseEvent
{
    Running,
    Stopped,
    Paused,
    Dead,
    SettingsChanged,
    DisplayChanged,
    DesktopChanged,
    FocusChanged,
    Connected,
    // The daemon detects the display turning off/on (sleep, session standby, lock/idle) and, like a
    // display change, unhooks itself first so the cursor is never left confined without the UI.
    Suspended,
    Resumed,
    // Outcome of a Load command: the daemon parsed the zones into its engine (payload is an
    // informative summary, e.g. "3 zones (3 main)"), or could not parse them. This is what makes
    // a Load-without-Run observable — the virtual-layout "simulate" flow relies on it, since no
    // Running event will ever follow. Older daemons simply never send these.
    Loaded,
    LoadFailed,
    // Edge-prober report (a <ProbeReport> document in the payload): emitted after every
    // virtual Load and on an explicit Probe command. See ProbeReport.TryParse.
    Probed,
    // The panic shortcut ran: the daemon has freed a cursor the user could not free with
    // the mouse, and is coming down. No payload — it does not know what the rescue should
    // mean, only that it happened, and knowing nothing is what lets it work with no UI
    // reachable. Distinct from Stopped, which says the same thing without saying why.
    Rescued,
}
public enum LittleBigMouseCommand
{
    Load,
    Run,
    Stop,
    Quit
}

public interface ILittleBigMouseService
{
    Task QuitAsync(CancellationToken token = default);
    Task StartAsync(ZonesLayout zonesLayout, CancellationToken token = default);
    Task StopAsync(CancellationToken token = default);
}