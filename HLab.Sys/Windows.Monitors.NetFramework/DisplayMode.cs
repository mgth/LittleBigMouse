/*
  HLab.Windows.Monitors
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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
using System.Windows;
using HLab.Windows.API;
using Newtonsoft.Json;

namespace HLab.Windows.Monitors
{
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
        public DisplayMode(NativeMethods.DEVMODE devmode)
        {
            DisplayOrientation = ((devmode.Fields & NativeMethods.DM.DisplayOrientation) != 0)?devmode.DisplayOrientation:0;
            Position = ((devmode.Fields & NativeMethods.DM.Position) != 0)?new Point(devmode.Position.x, devmode.Position.y):new Point(0,0);
            BitsPerPixel = ((devmode.Fields & NativeMethods.DM.BitsPerPixel) != 0)?devmode.BitsPerPel:0;
            Pels = ((devmode.Fields & (NativeMethods.DM.PelsWidth | NativeMethods.DM.PelsHeight)) != 0)? new Size(devmode.PelsWidth, devmode.PelsHeight):new Size(1,1);
            DisplayFlags = ((devmode.Fields & NativeMethods.DM.DisplayFlags) != 0)? devmode.DisplayFlags:0;
            DisplayFrequency = ((devmode.Fields & NativeMethods.DM.DisplayFrequency) != 0)? devmode.DisplayFrequency:0;
            DisplayFixedOutput = ((devmode.Fields & NativeMethods.DM.DisplayFixedOutput) != 0)? devmode.DisplayFixedOutput:0;
        }
    }
}
