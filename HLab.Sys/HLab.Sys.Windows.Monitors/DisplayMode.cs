/*
  HLab.Windows.Monitors
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.Monitors.

    HLab.Windows.Monitors is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.Monitors is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System.Runtime.Serialization;
using Avalonia;
using HLab.Sys.Windows.API;

namespace HLab.Sys.Windows.Monitors;

[DataContract]
public class DisplayMode
{
    [DataMember]
    public Point Position { get; }
    [DataMember]
    public int BitsPerPixel { get; }
    [DataMember]
    public Size Pels { get; }
    [DataMember]
    public int DisplayFlags { get; }
    [DataMember]
    public int DisplayFrequency { get; }
    [DataMember]
    public int DisplayFixedOutput { get; }
    [DataMember]
    public int DisplayOrientation { get; }
    internal DisplayMode(User32.DEVMODE dm)
    {
        DisplayOrientation = ((dm.Fields & User32.DM.DisplayOrientation) != 0)?dm.DisplayOrientation:0;
        Position = ((dm.Fields & User32.DM.Position) != 0)?new Point(dm.Position.x, dm.Position.y):new Point(0,0);
        BitsPerPixel = ((dm.Fields & User32.DM.BitsPerPixel) != 0)?dm.BitsPerPel:0;
        Pels = ((dm.Fields & (User32.DM.PelsWidth | User32.DM.PelsHeight)) != 0)? new Size(dm.PelsWidth, dm.PelsHeight):new Size(1,1);
        DisplayFlags = ((dm.Fields & User32.DM.DisplayFlags) != 0)? dm.DisplayFlags:0;
        DisplayFrequency = ((dm.Fields & User32.DM.DisplayFrequency) != 0)? dm.DisplayFrequency:0;
        DisplayFixedOutput = ((dm.Fields & User32.DM.DisplayFixedOutput) != 0)? dm.DisplayFixedOutput:0;
    }
}