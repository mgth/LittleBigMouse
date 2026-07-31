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
using System.Threading;
using System.Threading.Tasks;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.Ui.Avalonia.Remote;

public interface ILittleBigMouseClientService : ILittleBigMouseService
{
    event EventHandler<LittleBigMouseServiceEventArgs> DaemonEventReceived;
    LittleBigMouseEvent State { get; }

    /// <summary>
    /// Hand the daemon a layout to run right now, without adopting it: nothing is
    /// written to the store and nothing is written to the crash-recovery file, so a
    /// daemon restart comes back to the last <em>applied</em> layout. This is what
    /// the live-preview switch sends while the user edits.
    /// </summary>
    Task SendLiveAsync(ZonesLayout zonesLayout, CancellationToken token = default);

    /// <summary>
    /// Hand the daemon a panic shortcut to register now. It also travels inside the
    /// layout, which is what gets one to a standalone daemon at boot — but a shortcut
    /// recorded in the options has to take effect the moment it is recorded, and the
    /// answer (it is already taken) has to come back while the user is still looking at
    /// the setting rather than at the next Apply.
    /// </summary>
    Task SendShortcutAsync(string shortcut, CancellationToken token = default);
}