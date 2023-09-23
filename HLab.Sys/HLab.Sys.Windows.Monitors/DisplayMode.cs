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

namespace HLab.Sys.Windows.Monitors;

[DataContract]
public class DisplayMode
{
    /// <summary>
    /// Display position as of EnumDisplaySettingsEx 
    /// </summary>
    [DataMember] public Point Position { get; set; }

    /// <summary>
    /// Display bits per pixel (EnumDisplaySettingsEx)
    /// </summary>
    [DataMember] public uint BitsPerPixel { get; set; }

    /// <summary>
    ///  Size in pixels, of the visible device surface (EnumDisplaySettingsEx)
    /// </summary>
    [DataMember] public Size Pels { get; set; }

    /// <summary>
    /// 
    /// </summary>
    [DataMember] public int DisplayFlags { get; set; }

    /// <summary>
    /// Display frequency in Hertz (EnumDisplaySettingsEx)
    /// </summary>
    [DataMember] public int DisplayFrequency { get; set; }

    [DataMember] public int DisplayFixedOutput { get; set; }

    /// <summary>
    /// Display orientation (EnumDisplaySettingsEx)
    /// </summary>
    [DataMember] public int DisplayOrientation { get; set; }
}