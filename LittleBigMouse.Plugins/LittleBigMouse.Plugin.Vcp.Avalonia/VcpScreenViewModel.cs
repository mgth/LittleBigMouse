/*
  LittleBigMouse.Plugin.Vcp
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Vcp.

    LittleBigMouse.Plugin.Vcp is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Vcp is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System.Collections.ObjectModel;
using System.Diagnostics;
using System.Reactive.Linq;
using System.Windows.Input;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.ReactiveUI;
using HLab.Sys.Argyll;
using HLab.Sys.Windows.MonitorVcp;
using HLab.Sys.Windows.MonitorVcp.Avalonia;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugin.Vcp.Avalonia.Patterns;
using LiveChartsCore;
using LiveChartsCore.Kernel;
using LiveChartsCore.SkiaSharpView;
using LiveChartsCore.SkiaSharpView.Painting;
using LiveChartsCore.SkiaSharpView.Painting.Effects;
using ReactiveUI;
using SkiaSharp;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

public class VcpScreenViewModelDesign()
    : VcpScreenViewModel(vm => new TestPatternButtonViewModel(vm), null), IDesignViewModel;

public class VcpScreenViewModel : ViewModel<PhysicalMonitor>
{
   readonly IVcpService? _vcpService;

   // TODO : use reactive ui for collections
   public VcpScreenViewModel(
       Func<VcpScreenViewModel, TestPatternButtonViewModel> getButtonPattern,
       IVcpService? vcpService)
   {
      _vcpService = vcpService;

      TestPatterns.Add(getButtonPattern(this)
         .Set(TestPatternType.ContrastBoth)
         .Set(Colors.White, Colors.Black));
      TestPatterns.Add(getButtonPattern(this)
         .Set(TestPatternType.Contrast)
         .Set(Colors.White, Colors.Black));
      TestPatterns.Add(getButtonPattern(this)
         .Set(TestPatternType.Contrast)
         .Set(Colors.Black, Colors.White));

      TestPatterns.Add(getButtonPattern(this)
         .Set(TestPatternType.Circle)
         .Set(Color.FromRgb(0xFF, 0x80, 0x00), Colors.Black));

      TestPatterns.Add(getButtonPattern(this)
         .Set(TestPatternType.Circle)
         .Set(Color.FromRgb(0xFF, 0xFF, 0xFF), Colors.Black));

      TestPatterns.Add(getButtonPattern(this)
         .Set(TestPatternType.Gradient).SetRgb());

      TestPatterns.Add(getButtonPattern(this)
          .Set(TestPatternType.Gamma)
          .Set(Colors.White, Colors.Black)
          .Set(Orientation.Vertical).SetRgb());

      _vcp = this.WhenAnyValue(
              e => e.Model,
              selector: e => e is null ? null : _vcpService?.GetControl(e)?.Start())
          .ToProperty(this, e => e.Vcp);

      _brightnessVisibility = this.WhenAnyValue(
          e => e.Vcp.Brightness,
          selector: e => e != null)
          .ToProperty(this, e => e.BrightnessVisibility);

      _contrastVisibility = this.WhenAnyValue(
          e => e.Vcp.Contrast,
          selector: e => e != null)
          .ToProperty(this, e => e.ContrastVisibility);

      _gainVisibility = this.WhenAnyValue(
           e => e.Vcp.Gain,
           selector: e => e != null)
           .ToProperty(this, e => e.GainVisibility);

      _driveVisibility = this.WhenAnyValue(
           e => e.Vcp.Drive,
           selector: e => e != null)
           .ToProperty(this, e => e.DriveVisibility);

      // hidden while the capabilities probe is still running: no point offering
      // the forced activation before knowing what the monitor answered
      _anywayVisibility = this.WhenAnyValue(
           e => e.Vcp.Brightness,
           e => e.Vcp.Contrast,
           e => e.Vcp.Probing,
           (b, c, probing) => !probing && (b == null || c == null))
           .ToProperty(this, e => e.AnywayVisibility);

      _imageVisibility = this.WhenAnyValue(
           e => e.Vcp.Brightness,
           e => e.Vcp.Contrast,
           (b, c) => b != null || c != null)
           .ToProperty(this, e => e.ImageVisibility);

      // the advanced section is only meaningful once the monitor answered
      // with at least one adjustable level
      _advancedVisibility = this.WhenAnyValue(
           e => e.Vcp.Brightness,
           e => e.Vcp.Contrast,
           e => e.Vcp.Drive,
           (b, c, d) => b != null || c != null || d != null)
           .ToProperty(this, e => e.AdvancedVisibility);

      _probingVisibility = this.WhenAnyValue(
           e => e.Vcp.Probing)
           .ToProperty(this, e => e.ProbingVisibility);


      AnywayCommand = ReactiveCommand.Create(() => Vcp?.ActivateAnyway());
      ProbeBrightnessCommand = ReactiveCommand.Create(ProbeBrightness);

      _probeLut = this.WhenAnyValue(
          e => e.Model,
          selector: e =>
          {
             var lut = e is null ? null : _vcpService?.GetControl(e)?.ProbeLut();
             lut?.Load();
             return lut;
          })
          .ToProperty(this, e => e.ProbeLut);


      _series = this.WhenAnyValue(
            e => e.ProbeLut,
            selector: e => e?.SmoothLut)
            .Select(lut => new ISeries[]
            {
               new LineSeries<Tune>
               {
                     Values = lut,
                     Mapping = (tune, index) => new Coordinate(tune.Brightness, tune.Y),

                     Stroke = new SolidColorPaint(SKColors.Red),
                     GeometryStroke = null,
                     GeometryFill = null,
                     Fill = null, 

               },

               new LineSeries<Tune>
               {
                     Values = lut,
                     Mapping = (tune, index) => new Coordinate(tune.Brightness, tune.Red),

                     Stroke = new SolidColorPaint(SKColors.Red),
                     GeometryStroke = null,
                     GeometryFill = null,
                     Fill = null,
                     ScalesYAt = 1
               },
               new LineSeries<Tune>
               {
                     Values = lut,
                     Mapping = (tune, index) => new Coordinate(tune.Brightness, tune.Green),

                     Stroke = new SolidColorPaint(SKColors.Green),
                     GeometryStroke = null,
                     GeometryFill = null,
                     Fill = null,
                     ScalesYAt = 1
               },
               new LineSeries<Tune>
               {
                     Values = lut,
                     Mapping = (tune, index) => new Coordinate(tune.Brightness, tune.Blue),

                     Stroke = new SolidColorPaint(SKColors.Blue),
                     GeometryStroke = null,
                     GeometryFill = null,
                     Fill = null,
                     ScalesYAt = 1
               },

            })
            .ToProperty(this, e => e.Series);

      _xAxes = this.WhenAnyValue(
            e => e.ProbeLut,
            selector: e => e?.SortedLut)
            .Select(lut => new Axis[]
            {
               new Axis
               {
                  Name = "Brightness",
                  NamePaint = new SolidColorPaint(SKColors.Black),

                  LabelsPaint = new SolidColorPaint(SKColors.Blue),
                  TextSize = 10,

                  SeparatorsPaint = new SolidColorPaint(SKColors.LightSlateGray) { StrokeThickness = 2 }  ,
                  // no MaxLimit: autoscale follows the points as they land
                  MinLimit = 0,

               },
            })
            .ToProperty(this, e => e.XAxes);

      _yAxes = this.WhenAnyValue(
            e => e.ProbeLut,
            selector: e => e?.SortedLut)
            .Select(lut => new Axis[]
            {
               new Axis
               {
                  Name = "nits",
                  NamePaint = new SolidColorPaint(SKColors.Red), 

                  LabelsPaint = new SolidColorPaint(SKColors.Green), 
                  TextSize = 20,

                  SeparatorsPaint = new SolidColorPaint(SKColors.LightSlateGray)
                  {
                     StrokeThickness = 2,
                     PathEffect = new DashEffect([3, 3])
                  }            ,
                  // no MaxLimit: autoscale follows the points as they land
                  MinLimit = 0,


               },

               new Axis
               {
                  Name = "Gain",
                  NamePaint = new SolidColorPaint(SKColors.Black), 

                  LabelsPaint = new SolidColorPaint(SKColors.Blue), 
                  TextSize = 10,

                  SeparatorsPaint = new SolidColorPaint(SKColors.LightSlateGray) { StrokeThickness = 2 }  ,
                  // autoscale: fixed limits computed at bind time went stale
                  // (and threw on an empty lut) once live measurements landed
                  Position = LiveChartsCore.Measure.AxisPosition.End
               }

            })
            .ToProperty(this, e => e.YAxes);
   }

   public bool BrightnessVisibility => _brightnessVisibility.Value;
   readonly ObservableAsPropertyHelper<bool> _brightnessVisibility;

   public bool ContrastVisibility => _contrastVisibility.Value;
   readonly ObservableAsPropertyHelper<bool> _contrastVisibility;

   public bool GainVisibility => _gainVisibility.Value;
   readonly ObservableAsPropertyHelper<bool> _gainVisibility;

   public bool DriveVisibility => _driveVisibility.Value;
   readonly ObservableAsPropertyHelper<bool> _driveVisibility;

   public bool AnywayVisibility => _anywayVisibility.Value;
   readonly ObservableAsPropertyHelper<bool> _anywayVisibility;

   public bool ImageVisibility => _imageVisibility.Value;
   readonly ObservableAsPropertyHelper<bool> _imageVisibility;

   public bool AdvancedVisibility => _advancedVisibility.Value;
   readonly ObservableAsPropertyHelper<bool> _advancedVisibility;

   public bool ProbingVisibility => _probingVisibility.Value;
   readonly ObservableAsPropertyHelper<bool> _probingVisibility;

   public ICommand AnywayCommand { get; }
   public ICommand ProbeBrightnessCommand { get; }

   public VcpControl? Vcp => _vcp.Value;
   readonly ObservableAsPropertyHelper<VcpControl?> _vcp;

   public Color ColorA
   {
      get => _colorA;
      set => this.RaiseAndSetIfChanged(ref _colorA, value);
   }
   Color _colorA = Colors.White;

   public Color ColorB
   {
      get => _colorB;
      set => this.RaiseAndSetIfChanged(ref _colorB, value);
   }
   Color _colorB = Colors.Black;

   public Window? TestPatternPanel { get; set; } = null;

   /// <summary>Native-Wayland pattern viewer (lbm-pattern helper), when one is showing.</summary>
   public Process? NativePatternProcess { get; set; }
   public TestPattern? NativeShownPattern { get; set; }

   public void CloseNativePattern()
   {
      try
      {
         if (NativePatternProcess is { HasExited: false } process) process.Kill();
      }
      catch (Exception)
      {
      }
      NativePatternProcess = null;
      NativeShownPattern = null;
   }

   public ObservableCollection<TestPatternButtonViewModel> TestPatterns { get; } = new();

   public string Message => _message.Value;
   readonly ObservableAsPropertyHelper<string> _message;


   public ProbeLut? ProbeLut => _probeLut.Value;
   readonly ObservableAsPropertyHelper<ProbeLut?> _probeLut;
   public ArgyllProbe ArgyllProbe {get; } = new();

   public void ProbeLowLuminance()
   {
      if (!ArgyllProbe.Installed)
      {
         PleaseInstall();
         return;
      }


      new Thread(() =>
      {
         var level = Vcp.Brightness;

         var max = Vcp.Gain.Red.Max;
         var min = Vcp.Gain.Red.Min;

         var old = ProbeLut?.Luminance ?? 0;
         level.Value = 0;

         for (var i = max; i >= min; i--)
         {
            TuneWhitePoint(i);

            var t = ProbeLut.Current;

            if (ArgyllProbe.SpotRead())
            {
               t.Y = ArgyllProbe.ProbedColor.xyY.Y;
               t.x = ArgyllProbe.ProbedColor.xyY.x;
               t.y = ArgyllProbe.ProbedColor.xyY.y;
            }


            ProbeLut.RemoveLowBrightness(i);
            ProbeLut.Add(t);

            ProbeLut.Save();

            if (t.MinGain <= min) break;
         }

         if (ProbeLut is not null) ProbeLut.Luminance = old;
      }
      ).Start();
      return;
   }

   int _tuneRunning;

   void ProbeBrightness()
   {
      if (Vcp?.Brightness is null) return;

      var lut = ProbeLut;
      if (lut is null) return;

      // the panel's probe instance: its Message property is bound in the
      // calibration section, so progress and errors are actually visible
      var probe = ArgyllProbe;
      if (!probe.Installed)
      {
         probe.Message = "ArgyllCMS (spotread) not found — install argyllcms";
         return;
      }

      if (Interlocked.Exchange(ref _tuneRunning, 1) == 1) return;

      // the instrument needs several seconds to come up, then may ask for its
      // calibration position: say so before spotread produces its first output
      probe.Message = "Initializing instrument — calibration may be requested…";

      Task.Run(() =>
         {
            try
            {
               var level = Vcp.Brightness;

               for (var i = level.Min; i <= level.Max; i++)
               {
                  if(ProbeLut.SortedLut.Any(t => t.Brightness == i)) continue;

                  probe.Message = $"Measuring brightness {i} / {level.Max}…";

                  level.Value = i;

                  TuneWhitePoint();

                  var t = lut.Current;
                  if (probe.SpotRead())
                  {
                     t.Y = probe.ProbedColor.xyY.Y;
                     t.x = probe.ProbedColor.xyY.x;
                     t.y = probe.ProbedColor.xyY.y;
                  }

                  lut.RemoveBrightness(t.Brightness);
                  lut.Add(t);

                  ProbeLut?.Save();
               }

               probe.Message = "White point tuning done";
            }
            finally
            {
               Interlocked.Exchange(ref _tuneRunning, 0);
            }
         }
      );
      return;

      Task.Run(async () =>
      {
         for (uint channel = 0; channel < 3; channel++)
         {
            var level = Vcp.Gain.Channel(channel);
            //LineSeries line = new LineSeries();

            switch (channel)
            {
               case 0:
                  //line.Color = OxyColors.Red;
                  break;
               case 1:
                  //line.Color = OxyColors.Green;
                  break;
               case 2:
                  //line.Color = OxyColors.Blue;
                  break;
            }

            //  Curve.PlotModel.Series.Add(line);

            var max = level.Value;

            for (uint i = 32; i <= max; i++)
            {
               level.Value = i; await Task.Delay(1000);
               probe.SpotRead();
               var color = probe.ProbedColor;

               //line.Points.Add(new DataPoint(i, color.DeltaE00()));
               //Curve.Refresh();
            }

            var min = double.MaxValue;
            var minIdx = level.Value;
            //foreach (DataPoint dp in line.Points)
            //{
            //    if (dp.Y < min)
            //    {
            //        min = dp.Y;
            //        minIdx = (uint)dp.X;
            //    }
            //}

            level.Value = minIdx;
         }
      });
   }

   void Probe(ArgyllProbe probe, Color c, MonitorLevel level)
   {
      //LineSeries line = new LineSeries {Color = c};


      //Curve.PlotModel.Series.Add(line);

      for (var i = level.Min; i <= level.Max; i++)
      {
         level.Value = i;

         if (probe.SpotRead())
         {
            //line.Points.Add(new DataPoint(i, probe.ProbedColor.Lab.L));
            //Curve.Refresh();                    
         }
      }

   }
   public void Probe()
   {
      var probe = new ArgyllProbe();
      if (!probe.Installed)
      {
         PleaseInstall();
         return;
      }
      //Vcp.Brightness.SetToMax();
      //Vcp.Contrast.SetToMax();
      var red = Vcp.Gain.Red.Value;
      var green = Vcp.Gain.Green.Value;
      var blue = Vcp.Gain.Blue.Value;
      //Vcp.Brightness.SetToMax();
      //Vcp.Contrast.SetToMax();
      //Vcp.Gain.Red.SetToMax();
      //Vcp.Gain.Green.SetToMax();
      //Vcp.Gain.Blue.SetToMax();
      Task.Run(() =>
      {
         Vcp.Gain.Red.SetToMax();
         Vcp.Gain.Green.SetToMin();
         Vcp.Gain.Blue.SetToMin();
         Probe(probe, Colors.Red, Vcp.Contrast);

         Vcp.Gain.Red.SetToMin();
         Vcp.Gain.Green.SetToMax();
         Vcp.Gain.Blue.SetToMin();
         Probe(probe, Colors.Green, Vcp.Contrast);

         Vcp.Gain.Red.SetToMin();
         Vcp.Gain.Green.SetToMin();
         Vcp.Gain.Blue.SetToMax();
         Probe(probe, Colors.Blue, Vcp.Contrast);

         Vcp.Gain.Red.Value = red;
         Vcp.Gain.Green.Value = green;
         Vcp.Gain.Blue.Value = blue;
      }
      );
      return;

      Task.Run(async () =>
      {
         for (uint channel = 0; channel < 3; channel++)
         {
            var level = Vcp.Gain.Channel(channel);
            //LineSeries line = new LineSeries();

            switch (channel)
            {
               case 0:
                  //line.Color = OxyColors.Red;
                  break;
               case 1:
                  //line.Color = OxyColors.Green;
                  break;
               case 2:
                  //line.Color = OxyColors.Blue;
                  break;
            }

            //    Curve.PlotModel.Series.Add(line);

            var max = level.Value;

            for (uint i = 32; i <= max; i++)
            {
               level.Value = i; await Task.Delay(1000);

               if (probe.SpotRead())
               {
                  //line.Points.Add(new DataPoint(i,probe.ProbedColor.DeltaE00()));
                  //Curve.Refresh();                                            
               }
            }

            var min = double.MaxValue;
            var minIdx = level.Value;
            //foreach (DataPoint dp in line.Points)
            //{
            //    if (dp.Y < min)
            //    {
            //        min = dp.Y;
            //        minIdx = (uint)dp.X;
            //    }
            //}

            level.Value = minIdx;
         }
      });
   }

   double[,,] _tune = new double[0, 0, 0];

   public void Tune() => new Thread(TuneWhitePoint).Start();

   public void TuneWhitePoint() => TuneWhitePoint(0);

   public void TuneWhitePoint(uint max)
   {
      var gain = Vcp?.Gain;
      if (gain is null) return;

      var count = 6;
      uint channel = 0;

      _tune = new double[gain.Red.Max + 1, gain.Green.Max + 1, gain.Blue.Max + 1];

      var rgb = gain.GetValues();

      while (count > 0)
      {
         if (TuneWhitePoint(channel, ref rgb, max)) count = 5;
         else count--;

         channel++;
         channel %= 6;
      }

      gain.SetTo(rgb);
   }

   bool TuneWhitePoint(uint channel, ref uint[] rgb, uint maxLevel)
   {
      uint[] c;
      var min = new uint[3];
      var max = new uint[3];
      uint n = 0;

      switch (channel)
      {
         case 0:
            c = [0, 1, 2];
            n = 1;
            break;
         case 1:
            c = [1, 2, 0];
            n = 1;
            break;
         case 2:
            c = [2, 0, 1];
            n = 1;
            break;
         case 3: //0
            c = [0, 1, 2];
            n = 2;
            break;
         case 4: //1
            c = [1, 2, 0];
            n = 2;
            break;
         case 5: //2
            c = [2, 0, 1];
            n = 2;
            break;
         default:
            c = [0, 1, 2];
            n = 3;
            break;
      }


      for (var i = 0; i < 3; i++)
      {
         min[i] = Vcp.Gain.Channel(c[i]).Min;
         max[i] = maxLevel == 0 ? Vcp.Gain.Channel(c[i]).Max : maxLevel;
      }


      // keep one channel to its max
      switch (n)
      {
         case 1:
            if (rgb[c[0]] == max[0] && rgb[c[1]] < max[1] && rgb[c[2]] < max[2]) return false;
            break;
         case 2:
            if ((rgb[c[0]] == max[0] || rgb[c[1]] == max[1]) && rgb[c[2]] < max[2]) return false;
            break;
         case 3:
            return false;
      }

      uint[] oldGain = [rgb[c[0]], rgb[c[1]], rgb[c[2]]];

      var deltaE = Probe(rgb);

      while (rgb[c[0]] > min[0] && (n < 2 || rgb[c[1]] > min[1]) && (n < 3 || rgb[c[2]] > min[2]))
      {
         var old = deltaE;

         for (var i = 0; i < n; i++) rgb[c[i]]--;

         deltaE = Probe(rgb);

         if (deltaE > old) break;
      }

      while (rgb[c[0]] < max[0] && (n < 2 || rgb[c[1]] < max[1]) && (n < 3 || rgb[c[2]] < max[2]))
      {
         var old = deltaE;

         for (var i = 0; i < n; i++) rgb[c[i]]++;

         deltaE = Probe(rgb);

         if (deltaE <= old) continue;

         for (var i = 0; i < n; i++) rgb[c[i]]--;
         deltaE = Probe(rgb);
         break;
      }

      return (
          oldGain[0] != rgb[c[0]]
          || oldGain[1] != rgb[c[1]]
          || oldGain[2] != rgb[c[2]]
          );
   }

   double Probe(uint[] rgb)
   {
      var gain = Vcp?.Gain;
      if (gain is null) return 0;

      ref var tune = ref _tune[rgb[0], rgb[1], rgb[2]];
      if (tune != 0) return tune;

      gain.SetTo(rgb);

      Thread.Sleep(500);

      // shared panel instance: spotread's prompts stay visible in the bound Message
      var probe = ArgyllProbe;
      if (!probe.Installed)
      {
         PleaseInstall();
         return 0;
      }

      if (probe.SpotRead())
      {
         tune = probe.ProbedColor.DeltaE00();
      }
      return tune;
   }

   public void Save()
   {
      ProbeLut?.Save();
   }

   static void PleaseInstall()
   {
      // MessageBox.Show("Please install DispcalGUI & ArgyllCMS", "Calibration tools",
      //              MessageBoxButton.OK, MessageBoxImage.Exclamation);
   }


   public ICommand SwitchSourceCommand { get; }

   public ISeries[] Series => _series.Value;
   readonly ObservableAsPropertyHelper<ISeries[]> _series;
   public Axis[] XAxes => _xAxes.Value;
   readonly ObservableAsPropertyHelper<Axis[]> _xAxes;
   public Axis[] YAxes => _yAxes.Value;
   readonly ObservableAsPropertyHelper<Axis[]> _yAxes;

   public override void OnDispose()
   {
      CloseNativePattern();
      VcpExpendMonitor.Stop();
   }
}
