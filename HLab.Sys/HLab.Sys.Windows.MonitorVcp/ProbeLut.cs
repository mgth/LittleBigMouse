/*
  HLab.Windows.MonitorVcp
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.MonitorVcp.

    HLab.Windows.MonitorVcp is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.MonitorVcp is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

#nullable enable
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using System.Reactive.Linq;
using System.Xml.Serialization;
using DynamicData;
using DynamicData.Binding;
using HLab.Sys.Argyll;
using ReactiveUI;

namespace HLab.Sys.Windows.MonitorVcp;

public class ProbeLut : ReactiveObject
{
   //public ProbedColor DIlluminant { get; }

   readonly SourceList<Tune> _lut = new();


   readonly ReadOnlyObservableCollection<Tune> _sortedLut;
   public ReadOnlyObservableCollection<Tune> SortedLut => _sortedLut;

   readonly SourceList<Tune> _smoothLut = new();
   readonly ReadOnlyObservableCollection<Tune> _smoothLutCollection;
   public ReadOnlyObservableCollection<Tune> SmoothLut => _smoothLutCollection;

   internal ProbeLut(VcpControl vcp)
   {
      Vcp = vcp;

      _luminance = this.WhenAnyValue(
          e => e.Vcp.Brightness.Value,
          selector: e => GetLuminance()
      ).ToProperty(this, _ => _.Luminance);

     _lut
         .Connect()
         .Sort(SortExpressionComparer<Tune>.Ascending(t => t.Y))
         .ObserveOn(RxSchedulers.MainThreadScheduler)
         .Bind(out _sortedLut)
         .Subscribe();

     _smoothLut
         .Connect()
         .Sort(SortExpressionComparer<Tune>.Ascending(t => t.Y))
         .ObserveOn(RxSchedulers.MainThreadScheduler)
         .Bind(out _smoothLutCollection)
         .Subscribe();
   }

   public (double Slope, double Intercept) LinearRegression<T>(IEnumerable<T>list, Func<T,double> getX, Func<T,double> getY)
   {
      var sumX = 0.0;
      var sumY = 0.0;
      var sumXY = 0.0;
      var sumX2 = 0.0;
      var n = _sortedLut.Count;

      foreach (var tune in list)
      {
         var x = getX(tune);
         var y = getY(tune);

         sumX += x;
         sumY += y;
         sumXY += x * y;
         sumX2 += x * x;
      }

      var slope = (n * sumXY - sumX * sumY) / (n * sumX2 - sumX * sumX);
      var intercept = (sumY - slope * sumX) / n;

      return (slope, intercept);
   }

   public void GenerateSmoothedCurve()
   {
      _smoothLut.Clear();

      var(slopeY,interceptY) = LinearRegression(_sortedLut, t => t.Brightness, t => t.Y);
      var(slopeR,interceptR) = LinearRegression(_sortedLut, t => t.Brightness, t => t.Red);
      var(slopeG,interceptG) = LinearRegression(_sortedLut, t => t.Brightness, t => t.Green);
      var(slopeB,interceptB) = LinearRegression(_sortedLut, t => t.Brightness, t => t.Blue);

      var start = (int)_sortedLut.First().Brightness;
      var end = (int)_sortedLut.Last().Brightness;

      for (var i=start; i<end; i++ )
      {
         var tune = new Tune
         {
            Brightness = i,
            Y = (uint)(slopeY * i + interceptY),
            Red = (uint)(slopeR * i + interceptR),
            Green = (uint)(slopeG * i + interceptG),
            Blue = (uint)(slopeB * i + interceptB),
         };
         _smoothLut.Add(tune);
      }
   }

   public VcpControl Vcp { get; }

   public bool RemoveBrightness(double brightness)
   {
      var t = _sortedLut.FirstOrDefault(x => x.Brightness == brightness);
      if (t == null) return false;

      _lut.Remove(t);
      return true;
   }

   public bool RemoveLowBrightness(double maxGain)
   {
      var t = _sortedLut.FirstOrDefault(x => (x.Brightness == 0 && x.MaxGain == maxGain));
      if (t == null) return false;

      _lut.Remove(t);
      return true;
   }

   public void Add(Tune tune)
   {
      _lut.Add(tune);
   }

   public Tune FromLuminance(double luminance)
   {
      if (_sortedLut.Count == 0) return Current;

      Tune tSup = null;
      Tune tInf = null;

      var i = 0;
      for (; i < _sortedLut.Count && _sortedLut[i].Y < luminance; i++)
         tInf = _sortedLut[i];

      // luminance is more than monitor capabilities
      if (i >= _sortedLut.Count) return tInf;

      tSup = _sortedLut[i];

      if (tInf == null) return tSup;

      var dist = tSup.Y - tInf.Y;
      var ratio = (luminance - tInf.Y) / dist;

      var t = new Tune
      {
         Date = tSup.Date > tInf.Date ? tSup.Date : tInf.Date,

         Y = (uint)Math.Round(tInf.Y + (tSup.Y - tInf.Y) * ratio, 0),
         x = (uint)Math.Round(tInf.x + (tSup.x - tInf.x) * ratio, 0),
         y = (uint)Math.Round(tInf.y + (tSup.y - tInf.y) * ratio, 0),

         Brightness = (uint)Math.Round(tInf.Brightness + (tSup.Brightness - tInf.Brightness) * ratio, 0),
         Contrast = (uint)Math.Round(tInf.Contrast + (tSup.Contrast - tInf.Contrast) * ratio, 0),

         Red = (uint)Math.Round(tInf.Red + (tSup.Red - tInf.Red) * ratio, 0),
         Blue = (uint)Math.Round(tInf.Blue + (tSup.Blue - tInf.Blue) * ratio, 0),
         Green = (uint)Math.Round(tInf.Green + (tSup.Green - tInf.Green) * ratio, 0),
      };

      return t;
   }

   public Tune FromBrightness(double brightness)
   {
      if (_sortedLut is null) return Current;
      if (_sortedLut.Count == 0) return Current;

      Tune tSup = null;
      Tune tInf = null;

      var i = 0;
      for (; i < _sortedLut.Count && _sortedLut[i].Brightness < brightness; i++)
         tInf = _sortedLut[i];

      // luminance is more than monitor capabilities
      if (i >= _lut.Count) return tInf;

      tSup = _sortedLut[i];

      if (tInf == null) return tSup;

      var dist = tSup.Brightness - tInf.Brightness;
      var ratio = (brightness - tInf.Brightness) / dist;

      var t = new Tune
      {
         Date = tSup.Date > tInf.Date ? tSup.Date : tInf.Date,

         Y = (uint)Math.Round(tInf.Y + (tSup.Y - tInf.Y) * ratio, 0),
         x = (uint)Math.Round(tInf.x + (tSup.x - tInf.x) * ratio, 0),
         y = (uint)Math.Round(tInf.y + (tSup.y - tInf.y) * ratio, 0),

         Brightness = (uint)Math.Round(tInf.Brightness + (tSup.Brightness - tInf.Brightness) * ratio, 0),
         Contrast = (uint)Math.Round(tInf.Contrast + (tSup.Contrast - tInf.Contrast) * ratio, 0),

         Red = (uint)Math.Round(tInf.Red + (tSup.Red - tInf.Red) * ratio, 0),
         Blue = (uint)Math.Round(tInf.Blue + (tSup.Blue - tInf.Blue) * ratio, 0),
         Green = (uint)Math.Round(tInf.Green + (tSup.Green - tInf.Green) * ratio, 0),
      };

      return t;
   }

   double GetLuminance() => FromBrightness(Vcp.Brightness.Value).Y;

   void SetLuminance(double luminance)
   {
      var t = FromLuminance(luminance);
      Vcp.Brightness.Value = (uint)Math.Round(t.Brightness, 0);
      Vcp.Contrast.Value = (uint)Math.Round(t.Contrast, 0);
      Vcp.Gain.Red.Value = (uint)Math.Round(t.Red, 0);
      Vcp.Gain.Blue.Value = (uint)Math.Round(t.Blue, 0);
      Vcp.Gain.Green.Value = (uint)Math.Round(t.Green, 0);
   }

   public Tune Current
   {
      get
      {
         var vcp = Vcp;
         var gain = vcp?.Gain;
         if (vcp is null || gain is null) return new();
         return new()
         {
            Date = DateTime.Now,
            Brightness = vcp.Brightness?.Value??0,
            Contrast = vcp.Contrast?.Value??0,
            Red = gain.Red.Value,
            Blue = gain.Blue.Value,
            Green = gain.Green.Value,
         };
      }
   }

   public double Luminance
   {
      get => _luminance.Value;
      set => SetLuminance(value);
   }
   readonly ObservableAsPropertyHelper<double> _luminance;

   public double MaxLuminance =>
       (_sortedLut.Count == 0) ? 1 : _sortedLut.Last().Y;

   public double MinLuminance =>
       (_sortedLut.Count == 0) ? 0 : _sortedLut.First().Y;

   string GetConfigPath(bool create = false)
   {
      // Windows keeps the historical Mgth store; Linux follows the port's
      // convention: ~/.config/LittleBigMouse (no vendor segment).
      var path = OperatingSystem.IsWindows()
          ? Path.Combine(
              Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
              "Mgth", "LittleBigMouse")
          : Path.Combine(
              Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
              "LittleBigMouse");

      path = Path.Combine(path, Vcp.MonitorId);
      if (create) Directory.CreateDirectory(path);

      path = Path.Combine(path, "Luminance.xml");

      return path;
   }

   public void Save()
   {
      var serializer = new XmlSerializer(typeof(List<Tune>));
      using TextWriter writer = new StreamWriter(GetConfigPath(true));

      serializer.Serialize(writer, SortedLut.ToList());
   }

   public void Load()
   {
      var deserializer = new XmlSerializer(typeof(List<Tune>));
      try
      {
         using TextReader reader = new StreamReader(GetConfigPath());
         try
         {
            var lut = deserializer.Deserialize(reader) as IEnumerable<Tune>;
            if(lut is null) throw new Exception();

            _lut.Clear();
            foreach (var tune in lut) _lut.Add(tune);

            GenerateSmoothedCurve();
         }
         finally
         {
            reader.Close();
         }

      }
      catch (IOException ex) when (ex is FileNotFoundException or DirectoryNotFoundException)
      {
         _lut.Clear();

         _lut.Add(new()
         {
            Brightness = MinLuminance,
            Y = 0,
            Red = Vcp.Gain?.Red.Value ?? 0,
            Blue = Vcp.Gain?.Blue.Value ?? 0,
            Green = Vcp.Gain?.Green.Value ?? 0,
            Contrast = Vcp.Contrast?.Value ?? 0
         }
         );

         _lut.Add(new()
         {
            Brightness = MaxLuminance,
            Y = 160,
            Red = Vcp.Gain?.Red.Value ?? 0,
            Blue = Vcp.Gain?.Blue.Value ?? 0,
            Green = Vcp.Gain?.Green.Value ?? 0,
            Contrast = Vcp.Contrast?.Value ?? 0
         }
         );
      }
   }
}