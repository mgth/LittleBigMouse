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

using Avalonia;
using System;
using System.Runtime.Serialization;

namespace HLab.Sys.Windows.Monitors;

public interface IEdid
{
    string HKeyName { get; }
    string ManufacturerCode { get; }
    string ProductCode { get; }
    string Serial { get; }
    int Week { get; }
    int Year { get; }
    string Version { get; }
    bool Digital { get; }
    int BitDepth { get; }
    string VideoInterface { get; }
    Size PhysicalSize { get; }


    string Model { get; }

    string SerialNumber { get; }

    double Gamma { get; }

    bool DpmsStandbySupported { get; }
    bool DpmsSuspendSupported { get; }
    bool DpmsActiveOffSupported { get; }

    bool YCrCb444Support { get; }
    bool YCrCb422Support { get; }
    double sRGB { get; }

    double RedX { get; }
    double RedY { get; }
    double GreenX { get; }
    double GreenY { get; }
    double BlueX { get; }
    double BlueY { get; }
    double WhiteX { get; }
    double WhiteY { get; }
    int Checksum { get; }
}

public static class  EdidParser
{
    public static Edid Parse(string key, byte[] edid)
    {
      var e = new Edid
      {
         HKeyName = key
      };

      if (edid.Length <= 9) return e;

        e.ManufacturerCode = "" + (char)(64 + ((edid[8] >> 2) & 0x1F))
                              + (char)(64 + (((edid[8] << 3) | (edid[9] >> 5)) & 0x1F))
                              + (char)(64 + (edid[9] & 0x1F));

        if (edid.Length <= 11) return e;

        e.ProductCode = (edid[10] + (edid[11] << 8)).ToString("X4");

        if (edid.Length <= 15) return e;

        var serial = "";
        for (var i = 12; i <= 15; i++) serial = edid[i].ToString("X2") + serial;
        e.Serial = serial;

        if (edid.Length <= 16) return e;
        e.Week = edid[16];

        if (edid.Length < 18) return e;
        e.Year = edid[17] + 1990;

        if (edid.Length <= 19) return e;
        e.Version = edid[18] + "." + edid[19];

        if (edid.Length <= 20) return e;

        e.Digital = (edid[20] >> 7) == 1;
        if (e.Digital)
        {
            e.BitDepth = 4 + ((edid[20] & 0b01110000) >> 3);
            e.VideoInterface = (edid[20] & 0b1111) switch
            {
                0 => "undefined",
                2 => "HDMIa",
                3 => "HDMIb",
                4 => "MDDI",
                5 => "DisplayPort",
                _ => (edid[20] & 0b1111).ToString("X1")
            };

        }
        else
        {

        }
        if (edid.Length <= 21) return e;

        e.PhysicalWidth = edid[21] * 10;

        if (edid.Length <= 22) return e;

        e.PhysicalHeight = edid[22] * 10;


        if (edid.Length <= 23) return e;

        if (edid[23] < 255)
        {
            e.Gamma = 1.0 + edid[23] / 100.0;
        }
        else
        {
            // todo : else check DI-EXT block
        }
        if (edid.Length <= 24) return e;
        e.DpmsStandbySupported = (edid[24] & (0b1 << 7)) > 0;
        e.DpmsSuspendSupported = (edid[24] & (0b1 << 6)) > 0;
        e.DpmsActiveOffSupported = (edid[24] & (0b1 << 5)) > 0;

        if (e.Digital)
        {
            e.YCrCb422Support = (edid[24] & (0b1 << 4)) > 0;
            e.YCrCb444Support = (edid[24] & (0b1 << 3)) > 0;
        }
        else
        {
            // todo : analog display
        }

        if (edid.Length <= 34) return e;

        if ((edid[24] & (0b1 << 2)) > 0)
        {
            var redX = ((edid[25] >> 6) & 0b11) | (((int)edid[27]) << 2);
            var redY = ((edid[25] >> 4) & 0b11) | (((int)edid[28]) << 2);
            var greenX = ((edid[25] >> 2) & 0b11) | (((int)edid[29]) << 2);
            var greenY = (edid[25] & 0b11) | (((int)edid[30]) << 2);

            var blueX = ((edid[26] >> 6) & 0b11) | (((int)edid[31]) << 2);
            var blueY = ((edid[26] >> 4) & 0b11) | (((int)edid[32]) << 2);
            var whiteX = ((edid[26] >> 2) & 0b11) | (((int)edid[33]) << 2);
            var whiteY = (edid[26] & 0b11) | (((int)edid[34]) << 2);

            e.RedX = ((double)redX) / 1024.0;
            e.RedY = ((double)redY) / 1024.0;
            e.GreenX = ((double)greenX) / 1024.0;
            e.GreenY = ((double)greenY) / 1024.0;
            e.BlueX = ((double)blueX) / 1024.0;
            e.BlueY = ((double)blueY) / 1024.0;
            e.WhiteX = ((double)whiteX) / 1024.0;
            e.WhiteY = ((double)whiteY) / 1024.0;
        }


        if (edid.Length <= 68) return e;

        e.PhysicalWidth =  ((edid[68] & 0xF0) << 4) + edid[66];
        e.PhysicalHeight = ((edid[68] & 0x0F) << 8) + edid[67];

        e.Model = Block((char)0xFC, edid);

        e.SerialNumber = Block((char)0xFF, edid);

        if (edid.Length <= 127) return e;
        e.Checksum = edid[127];

      return e;
    }
    static string Block(char code, byte[] edid)
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
             if (c == (char)0x00) break;
             s += c;
          }
          return s;
       }
       return "";
    }

 
}


public class Edid
{
    [DataMember] 
    public string HKeyName { get; set; }
    [DataMember]
    public string ProductCode { get; set; }
    [DataMember]
    public string Serial { get; set; }
    [DataMember]
    public double PhysicalWidth { get; set; }
    [DataMember]
    public double PhysicalHeight { get; set; }
    [DataMember]
    public string ManufacturerCode { get; set; }
    [DataMember]
    public string Model { get; set; }
    [DataMember]
    public string SerialNumber { get; set; }
    [DataMember]
    public int Week { get; set; }
    [DataMember]
    public int Year { get; set; }
    [DataMember]
    public string Version { get; set; }
    [DataMember]
    public bool Digital { get; set; }
    [DataMember]        
    public int BitDepth { get; set; }
    [DataMember]
    public string VideoInterface { get; set; }
    [DataMember]
    public double Gamma { get; set; }
    [DataMember]
    public bool DpmsStandbySupported { get; set; }
    [DataMember]
    public bool DpmsSuspendSupported { get; set; }
    [DataMember]
    public bool DpmsActiveOffSupported { get; set; }
    [DataMember]
    public bool YCrCb444Support { get; set; }
    [DataMember]
    public bool YCrCb422Support { get; set; }
    [DataMember]
    public double sRGB { get; set; }

    [DataMember]    
    public double RedX { get; set; }
    [DataMember]
    public double RedY { get; set; }
    [DataMember]
    public double GreenX { get; set; }
    [DataMember]
    public double GreenY { get; set; }
    [DataMember]
    public double BlueX { get; set; }
    [DataMember]
    public double BlueY { get ; set; }
    [DataMember]
    public double WhiteX { get; set; }
    [DataMember]
    public double WhiteY { get; set; }
    [DataMember]
    public int Checksum { get; set; }
}