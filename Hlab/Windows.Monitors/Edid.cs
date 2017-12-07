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
using System.Windows;

namespace HLab.Windows.Monitors
{
    public class Edid
    {

        public Edid(byte[] edid)
        {
            if (edid.Length < 10) return;

                ManufacturerCode = "" + (char)(64 + ((edid[8] >> 2) & 0x1F))
                                   + (char)(64 + (((edid[8] << 3) | (edid[9] >> 5)) & 0x1F))
                                   + (char)(64 + (edid[9] & 0x1F));

            if (edid.Length < 12) return;

                ProductCode = (edid[10] + (edid[11] << 8)).ToString("X4");

            if (edid.Length < 16) return;

                var serial = "";
                for (var i = 12; i <= 15; i++) serial = edid[i].ToString("X2") + serial;
                Serial = serial;

            if (edid.Length <= 68) return;

                PhysicalSize = new Size(
                    ((edid[68] & 0xF0) << 4) + edid[66],
                    ((edid[68] & 0x0F) << 8) + edid[67]
                );

            Model = Block((char)0xFC,edid);

            SerialNo = Block((char)0xFF,edid);

        }

    

        public string ProductCode { get; }
        public string Serial { get; }
        public Size PhysicalSize { get; }

        public string ManufacturerCode { get; }

        public string Model { get; }

        public string SerialNo { get; }


        private string Block(char code, byte[] edid)
        {
            for (var i = 54; i <= 108; i += 18)
            {
                if (i >= edid.Length || edid[i] != 0 || edid[i + 1] != 0 || edid[i + 2] != 0 ||
                    edid[i + 3] != code) continue;
                var s = "";
                for (var j = i + 5; j < i + 18; j++)
                {
                    var c = (char)edid[j];
                    if (c == (char)0x0A) break;
                    s += c;
                }
                return s;
            }
            return "";
        }

    }
}
