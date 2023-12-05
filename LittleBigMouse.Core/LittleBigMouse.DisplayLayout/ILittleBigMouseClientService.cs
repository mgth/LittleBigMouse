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
using LittleBigMouse.Zoning;

namespace LittleBigMouse.DisplayLayout;

public class LittleBigMouseServiceEventArgs(LittleBigMouseEvent evt, string payload) : EventArgs
{
    public LittleBigMouseEvent Event { get; } = evt;
    public string Payload { get; } = payload;
}

public interface ILittleBigMouseClientService : ILittleBigMouseService
{
    event EventHandler<LittleBigMouseServiceEventArgs> DaemonEventReceived;
    LittleBigMouseEvent State { get; }
}