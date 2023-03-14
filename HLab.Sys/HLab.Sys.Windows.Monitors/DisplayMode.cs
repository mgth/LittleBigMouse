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
using static HLab.Sys.Windows.API.WinGdi;

namespace HLab.Sys.Windows.Monitors;

[DataContract]
public class DisplayMode
{
    [DataMember]
    public Point Position { get; }
    [DataMember]
    public uint BitsPerPixel { get; }
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
    internal DisplayMode(DevMode dm)
    {
        DisplayOrientation = (int)(((dm.Fields & DisplayModeFlags.DisplayOrientation) != 0)?dm.DisplayOrientation:DevMode.DisplayOrientationEnum.Default);
        Position = ((dm.Fields & DisplayModeFlags.Position) != 0)?new Point(dm.Position.X, dm.Position.Y):new Point(0,0);
        BitsPerPixel = ((dm.Fields & DisplayModeFlags.BitsPerPixel) != 0)?dm.BitsPerPel:0;
        Pels = ((dm.Fields & (DisplayModeFlags.PixelsWidth | DisplayModeFlags.PixelsHeight)) != 0)? new Size(dm.PixelsWidth, dm.PixelsHeight):new Size(1,1);
        DisplayFlags = (int)(((dm.Fields & DisplayModeFlags.DisplayFlags) != 0) ? dm.DisplayFlags : 0);
        DisplayFrequency = (int)(((dm.Fields & DisplayModeFlags.DisplayFrequency) != 0)? dm.DisplayFrequency:0);
        DisplayFixedOutput = (int)(((dm.Fields & DisplayModeFlags.DisplayFixedOutput) != 0)? dm.DisplayFixedOutput:0);
    }
}