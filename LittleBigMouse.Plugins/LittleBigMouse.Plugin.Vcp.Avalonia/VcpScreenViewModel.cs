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
using System.Reactive.Concurrency;
using System.Reactive.Linq;
using System.Windows.Input;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Immutable;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.ReactiveUI;
using HLab.Sys.Argyll;
using HLab.Sys.Windows.MonitorVcp;
using HLab.Sys.Windows.MonitorVcp.Avalonia;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugin.Vcp.Avalonia.Patterns;
using LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;
using LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;
using LiveChartsCore;
using LiveChartsCore.Kernel;
using LiveChartsCore.SkiaSharpView;
using LiveChartsCore.SkiaSharpView.Painting;
using LiveChartsCore.SkiaSharpView.Painting.Effects;
using ReactiveUI;
using SkiaSharp;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

public class VcpScreenViewModelDesign()
    : VcpScreenViewModel(vm => new TestPatternButtonViewModel(vm), null, null, null, null), IDesignViewModel;

public record ObserverChoice(ArgyllProbe.ObserverEnum Value, string Label);

public record SpeedChoice(string Label, bool Adaptive, int SettleMs);

public class ProbeLogEntry
{
   public bool R { get; init; }
   public bool G { get; init; }
   public bool B { get; init; }
   public string Arrow { get; init; } = "";
   public string Delta { get; init; } = "";
   public string Verdict { get; init; } = "";
   public IBrush VerdictBrush { get; init; } = Brushes.Transparent;
}

public class VcpScreenViewModel : ViewModel<PhysicalMonitor>
{
   readonly IVcpService? _vcpService;

   // TODO : use reactive ui for collections
   public VcpScreenViewModel(
       Func<VcpScreenViewModel, TestPatternButtonViewModel> getButtonPattern,
       IVcpService? vcpService,
       ISamsungTizenService? samsungTizenService,
       IHisenseVidaaService? hisenseVidaaService,
       ILayoutOptions? layoutOptions)
   {
      _vcpService = vcpService;

      // experimental gate: Argyll calibration and the smart-TV test tooling
      // stay hidden unless enabled in the application options
      _experimentalEnabled = (layoutOptions is null
              ? Observable.Return(false)
              : layoutOptions.WhenAnyValue(o => o.ExperimentalFeatures))
          .ToProperty(this, e => e.ExperimentalEnabled);
      Samsung = new SamsungControlViewModel(samsungTizenService);
      Hisense = new HisenseControlViewModel(hisenseVidaaService);

      this.WhenAnyValue(e => e.Model)
          .Subscribe(Samsung.SetMonitor);
      this.WhenAnyValue(e => e.Model)
          .Subscribe(Hisense.SetMonitor);

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

      // Resolving a physical DDC/CI channel may enumerate DRM/sysfs or Win32
      // monitor handles. Keep the entire lookup off the UI thread; VcpControl
      // itself continues its capabilities probe in the background.
      _vcp = this.WhenAnyValue(e => e.Model)
          .Select(m => m is null || _vcpService is null
              ? Observable.Return<VcpControl?>(null)
              : Observable.FromAsync(ct => _vcpService.GetControlAsync(m, ct))
                  .Catch<VcpControl?, Exception>(error =>
                  {
                     Console.Error.WriteLine($"VCP: unable to resolve {m.Id}: {error.Message}");
                     return Observable.Return<VcpControl?>(null);
                  }))
          .Switch()
          .ObserveOn(RxSchedulers.MainThreadScheduler)
          .Select(control => control?.Start())
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

      // the advanced section is only meaningful once the monitor answered with
      // at least one adjustable level; without the experimental gate it only
      // holds the drive faders, calibration being hidden
      _advancedVisibility = this.WhenAnyValue(
           e => e.Vcp.Brightness,
           e => e.Vcp.Contrast,
           e => e.Vcp.Drive,
           e => e.ExperimentalEnabled,
           (b, c, d, experimental) => d != null || (experimental && (b != null || c != null)))
           .ToProperty(this, e => e.AdvancedVisibility);

      _calibrationVisibility = this.WhenAnyValue(
           e => e.BrightnessVisibility,
           e => e.ExperimentalEnabled,
           (brightness, experimental) => brightness && experimental)
           .ToProperty(this, e => e.CalibrationVisibility);

      _probingVisibility = this.WhenAnyValue(
           e => e.Vcp.Probing)
           .ToProperty(this, e => e.ProbingVisibility);

      _selectedSpeed = Speeds[1];

      // the speed preset drives spotread's adaptive integration flag
      this.WhenAnyValue(e => e.SelectedSpeed)
          .Subscribe(s => { if (s is not null) ArgyllProbe.Adaptive = s.Adaptive; });

      // persisted calibration settings: common file, overridden per monitor
      this.WhenAnyValue(e => e.Model)
          .Subscribe(m => { if (m is not null) LoadCalibrationSettings(m.Id); });

      this.WhenAnyValue(
           e => e.ArgyllProbe.ColorTemp,
           e => e.ArgyllProbe.Observer,
           e => e.SelectedSpeed)
          .Subscribe(_ => SaveCalibrationValues());

      this.WhenAnyValue(e => e.TestPairs)
          .Subscribe(_ => SaveMonitorCalibration());

      this.WhenAnyValue(e => e.UseCustomSettings)
          .Subscribe(OnUseCustomChanged);

      // what will actually be launched, kept honest by recomputing from the probe
      _spotreadCommand = this.WhenAnyValue(
           e => e.ArgyllProbe.Observer,
           e => e.ArgyllProbe.ColorTemp,
           e => e.SelectedSpeed,
           (o, t, s) => $"spotread{ArgyllProbe.SpotReadArgs}   →   D{t / 100:0} target")
           .ToProperty(this, e => e.SpotreadCommand);


      AnywayCommand = ReactiveCommand.Create(() => Vcp?.ActivateAnyway());
      ProbeBrightnessCommand = ReactiveCommand.Create(ProbeBrightness);

      ClearLutCommand = ReactiveCommand.Create(() =>
      {
         var lut = ProbeLut;
         if (lut is null) return;
         lut.Clear();
         lut.Save();
         LastMeasure = "";
         ProbeLog.Clear();
         ProbeVisible = false;
      });

      StopTuneCommand = ReactiveCommand.Create(() =>
      {
         ArgyllProbe.Abort();
         ArgyllProbe.Message = "Stopping…";
      });

      _probeLut = this.WhenAnyValue(
          e => e.Vcp,
          selector: control =>
          {
             // Reuse the asynchronously resolved control. Calling the service
             // here used to put a second physical-monitor lookup on the UI thread.
             var lut = control?.ProbeLut();
             lut?.Load();
             return lut;
          })
          .ToProperty(this, e => e.ProbeLut);


      _series = this.WhenAnyValue(
            e => e.ProbeLut)
            .Select(lut => new ISeries[]
            {
               new LineSeries<Tune>
               {
                     Values = lut?.SmoothLut,
                     Mapping = (tune, index) => new Coordinate(tune.Brightness, tune.Y),

                     Stroke = new SolidColorPaint(SKColors.White),
                     GeometryStroke = null,
                     GeometryFill = null,
                     Fill = null,

               },

               new LineSeries<Tune>
               {
                     Values = lut?.SmoothLut,
                     Mapping = (tune, index) => new Coordinate(tune.Brightness, tune.Red),

                     Stroke = new SolidColorPaint(SKColors.Red),
                     GeometryStroke = null,
                     GeometryFill = null,
                     Fill = null,
                     ScalesYAt = 1
               },
               new LineSeries<Tune>
               {
                     Values = lut?.SmoothLut,
                     Mapping = (tune, index) => new Coordinate(tune.Brightness, tune.Green),

                     Stroke = new SolidColorPaint(SKColors.Green),
                     GeometryStroke = null,
                     GeometryFill = null,
                     Fill = null,
                     ScalesYAt = 1
               },
               new LineSeries<Tune>
               {
                     Values = lut?.SmoothLut,
                     Mapping = (tune, index) => new Coordinate(tune.Brightness, tune.Blue),

                     Stroke = new SolidColorPaint(SKColors.Blue),
                     GeometryStroke = null,
                     GeometryFill = null,
                     Fill = null,
                     ScalesYAt = 1
               },

               // measured ΔE00 after tuning, one point per brightness step
               new LineSeries<Tune>
               {
                     Values = lut?.SortedLut,
                     Mapping = (tune, index) => new Coordinate(tune.Brightness, tune.DeltaE),

                     Stroke = new SolidColorPaint(SKColors.Orange),
                     GeometryStroke = null,
                     GeometryFill = new SolidColorPaint(SKColors.Orange),
                     GeometrySize = 6,
                     Fill = null,
                     ScalesYAt = 2
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
               },

               new Axis
               {
                  Name = "ΔE00",
                  NamePaint = new SolidColorPaint(SKColors.Orange),

                  LabelsPaint = new SolidColorPaint(SKColors.Orange),
                  TextSize = 10,

                  SeparatorsPaint = null,
                  MinLimit = 0,
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

   /// <summary>Experimental features enabled in the application options.</summary>
   public bool ExperimentalEnabled => _experimentalEnabled.Value;
   readonly ObservableAsPropertyHelper<bool> _experimentalEnabled;

   /// <summary>Argyll calibration section: needs a brightness level and the experimental gate.</summary>
   public bool CalibrationVisibility => _calibrationVisibility.Value;
   readonly ObservableAsPropertyHelper<bool> _calibrationVisibility;

   public bool ProbingVisibility => _probingVisibility.Value;
   readonly ObservableAsPropertyHelper<bool> _probingVisibility;

   public ICommand AnywayCommand { get; }
   public ICommand ProbeBrightnessCommand { get; }
   public ICommand ClearLutCommand { get; }
   public ICommand StopTuneCommand { get; }

   /// <summary>Local-network controls for Samsung Tizen monitors such as the Odyssey G80SD.</summary>
   public SamsungControlViewModel Samsung { get; }
   public HisenseControlViewModel Hisense { get; }

   /// <summary>True while a white point sweep runs — swaps the Tune button for Stop.</summary>
   public bool TuneRunning
   {
      get => _tuneRunningUi;
      private set => this.RaiseAndSetIfChanged(ref _tuneRunningUi, value);
   }
   bool _tuneRunningUi;

   /// <summary>Last tuned-and-measured brightness step of the running sweep.</summary>
   public string LastMeasure
   {
      get => _lastMeasure;
      private set => this.RaiseAndSetIfChanged(ref _lastMeasure, value);
   }
   string _lastMeasure = "";

   static readonly IBrush ImprovedBrush = new ImmutableSolidColorBrush(Color.FromRgb(0x55, 0xB9, 0x4F));
   static readonly IBrush WorseBrush = new ImmutableSolidColorBrush(Color.FromRgb(0xE0, 0x52, 0x4E));
   static readonly IBrush NeutralBrush = new ImmutableSolidColorBrush(Color.FromRgb(0x90, 0x90, 0x90));

   const int ProbeLogLength = 6;

   public bool ProbeVisible { get => _probeVisible; private set => this.RaiseAndSetIfChanged(ref _probeVisible, value); }
   bool _probeVisible;

   /// <summary>Rolling history of the tuning spot reads, newest first.</summary>
   public ObservableCollection<ProbeLogEntry> ProbeLog { get; } = new();

   /// <summary>One entry per spot read: channels touched, direction, ΔE00 before → after, verdict.</summary>
   void ReportProbe(uint[] channels, int n, string arrow, double previous, double current, bool revert = false)
   {
      var r = false; var g = false; var b = false;
      for (var i = 0; i < n; i++)
         switch (channels[i])
         {
            case 0: r = true; break;
            case 1: g = true; break;
            case 2: b = true; break;
         }

      string delta, verdict;
      IBrush brush;

      if (arrow == "")
      {
         // baseline reading of a new cycle: nothing changed yet
         delta = $"ΔE00 {current:0.00}";
         (verdict, brush) = ("baseline", NeutralBrush);
      }
      else
      {
         delta = $"ΔE00 {previous:0.00} → {current:0.00}";
         (verdict, brush) = revert
            ? ("revert", NeutralBrush)
            : current.CompareTo(previous) switch
            {
               < 0 => ("improved", ImprovedBrush),
               > 0 => ("worse", WorseBrush),
               _ => ("same", NeutralBrush),
            };
      }

      var entry = new ProbeLogEntry
      {
         R = r, G = g, B = b,
         Arrow = arrow,
         Delta = delta,
         Verdict = verdict,
         VerdictBrush = brush,
      };

      // called from the measurement task: the observable collection feeds an
      // ItemsControl and must only mutate on the UI thread
      RxSchedulers.MainThreadScheduler.Schedule(() =>
      {
         ProbeLog.Insert(0, entry);
         while (ProbeLog.Count > ProbeLogLength) ProbeLog.RemoveAt(ProbeLog.Count - 1);
         ProbeVisible = true;
      });
   }

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

   /// <summary>Measurement speed presets: spotread adaptive integration ± settle delay after a gain write.</summary>
   public IReadOnlyList<SpeedChoice> Speeds { get; } =
   [
      new("Fast", false, 200),
      new("Normal", true, 500),
      new("Careful", true, 1000),
   ];

   public SpeedChoice SelectedSpeed
   {
      get => _selectedSpeed;
      set => this.RaiseAndSetIfChanged(ref _selectedSpeed, value);
   }
   SpeedChoice _selectedSpeed;

   /// <summary>Also test channel pairs (RG, GB, BR): needed on monitors with coupled gains, twice slower. Always per-monitor.</summary>
   public bool TestPairs
   {
      get => _testPairs;
      set => this.RaiseAndSetIfChanged(ref _testPairs, value);
   }
   bool _testPairs = true;

   /// <summary>Use this monitor's own white point / observer / speed instead of the common ones.</summary>
   public bool UseCustomSettings
   {
      get => _useCustomSettings;
      set => this.RaiseAndSetIfChanged(ref _useCustomSettings, value);
   }
   bool _useCustomSettings;

   string? _monitorId;
   CalibrationSettings? _globalSettings;
   CalibrationSettings? _monitorSettings;
   bool _settingsLoading;

   void LoadCalibrationSettings(string monitorId)
   {
      _settingsLoading = true;
      try
      {
         _monitorId = monitorId;
         _globalSettings = CalibrationSettingsStore.LoadGlobal();
         _monitorSettings = CalibrationSettingsStore.LoadMonitor(monitorId);

         TestPairs = _monitorSettings?.TestPairs ?? true;
         UseCustomSettings = _monitorSettings?.UseCustom ?? false;

         // DisplayCAL's config, loaded by the probe's constructor, stays the
         // fallback until something has been persisted
         var source = UseCustomSettings ? _monitorSettings : _globalSettings;
         if (source is not null) ApplyCalibrationSettings(source);
      }
      finally
      {
         _settingsLoading = false;
      }
   }

   void ApplyCalibrationSettings(CalibrationSettings settings)
   {
      ArgyllProbe.ColorTemp = settings.ColorTemp;
      if (Enum.TryParse<ArgyllProbe.ObserverEnum>(settings.Observer, out var observer))
         ArgyllProbe.Observer = observer;
      SelectedSpeed = Speeds.FirstOrDefault(s => s.Label == settings.Speed) ?? Speeds[1];
   }

   CalibrationSettings SnapshotCalibrationValues(CalibrationSettings? into = null)
   {
      var s = into ?? new CalibrationSettings();
      s.ColorTemp = ArgyllProbe.ColorTemp;
      s.Observer = ArgyllProbe.Observer.ToString();
      s.Speed = SelectedSpeed?.Label ?? "Normal";
      return s;
   }

   void SaveCalibrationValues()
   {
      if (_settingsLoading || _monitorId is null) return;

      if (UseCustomSettings)
      {
         SaveMonitorCalibration();
      }
      else
      {
         _globalSettings = SnapshotCalibrationValues(_globalSettings);
         CalibrationSettingsStore.SaveGlobal(_globalSettings);
      }
   }

   void SaveMonitorCalibration()
   {
      if (_settingsLoading || _monitorId is null) return;

      // first write seeds the custom values from the current effective ones;
      // while UseCustom is off they are left untouched as a seed for later
      _monitorSettings ??= SnapshotCalibrationValues();
      _monitorSettings.UseCustom = UseCustomSettings;
      _monitorSettings.TestPairs = TestPairs;
      if (UseCustomSettings) SnapshotCalibrationValues(_monitorSettings);

      CalibrationSettingsStore.SaveMonitor(_monitorId, _monitorSettings);
   }

   void OnUseCustomChanged(bool useCustom)
   {
      if (_settingsLoading || _monitorId is null) return;

      if (useCustom)
      {
         // resume from this monitor's stored values (or seed them from the
         // current common ones so the toggle never jumps)
         _monitorSettings ??= SnapshotCalibrationValues();
         _settingsLoading = true;
         try { ApplyCalibrationSettings(_monitorSettings); }
         finally { _settingsLoading = false; }
      }
      else if (_globalSettings is not null)
      {
         _settingsLoading = true;
         try { ApplyCalibrationSettings(_globalSettings); }
         finally { _settingsLoading = false; }
      }

      SaveMonitorCalibration();
   }

   /// <summary>Observer choices for the calibration combo, session-scoped edits over the DisplayCAL defaults.</summary>
   public IReadOnlyList<ObserverChoice> Observers { get; } =
   [
      new(ArgyllProbe.ObserverEnum.CIE_1931_2, "CIE 1931 2°"),
      new(ArgyllProbe.ObserverEnum.CIE_1964_10, "CIE 1964 10°"),
      new(ArgyllProbe.ObserverEnum.CIE_2012_2, "CIE 2012 2°"),
      new(ArgyllProbe.ObserverEnum.CIE_2012_10, "CIE 2012 10°"),
      new(ArgyllProbe.ObserverEnum.SB_1955_2, "Stiles-Burch 1955 2°"),
      new(ArgyllProbe.ObserverEnum.JV_1978_2, "Judd-Vos 1978 2°"),
      new(ArgyllProbe.ObserverEnum.Shaw, "Shaw-Fairchild 1997 2°"),
   ];

   public string SpotreadCommand => _spotreadCommand.Value;
   readonly ObservableAsPropertyHelper<string> _spotreadCommand;

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

      probe.ResetAbort();
      TuneRunning = true;

      // the instrument needs several seconds to come up, then may ask for its
      // calibration position: say so before spotread produces its first output
      probe.Message = "Initializing instrument — calibration may be requested…";

      Task.Run(() =>
         {
            try
            {
               var level = Vcp.Brightness;

               for (var i = level.Min; i <= level.Max && !probe.Aborted; i++)
               {
                  if(ProbeLut.SortedLut.Any(t => t.Brightness == i)) continue;

                  probe.Message = $"Measuring brightness {i} / {level.Max}…";

                  level.Value = i;

                  TuneWhitePoint();

                  // aborted mid-tuning: don't record a half-tuned point
                  if (probe.Aborted) break;

                  var t = lut.Current;
                  if (probe.SpotRead())
                  {
                     t.Y = probe.ProbedColor.xyY.Y;
                     t.x = probe.ProbedColor.xyY.x;
                     t.y = probe.ProbedColor.xyY.y;
                     t.DeltaE = probe.ProbedColor.DeltaE00();
                  }
                  else if (probe.Aborted) break;

                  lut.RemoveBrightness(t.Brightness);
                  lut.Add(t);

                  ProbeLut?.Save();

                  LastMeasure = $"B {t.Brightness:0} → {t.Y:0.0} cd/m² · R {t.Red:0} G {t.Green:0} B {t.Blue:0} · ΔE00 {t.DeltaE:0.00}";
               }

               probe.Message = probe.Aborted ? "Tuning stopped" : "White point tuning done";
            }
            finally
            {
               Interlocked.Exchange(ref _tuneRunning, 0);
               RxSchedulers.MainThreadScheduler.Schedule(() => TuneRunning = false);
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

      while (count > 0 && !ArgyllProbe.Aborted)
      {
         if (TuneWhitePoint(channel, ref rgb, max)) count = 5;
         else count--;

         channel++;
         // phases 3-5 move channel pairs, for monitors whose gains interact —
         // skippable, they double the cycle
         channel %= TestPairs ? 6u : 3u;
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
      ReportProbe(c, (int)n, "", deltaE, deltaE);

      while (!ArgyllProbe.Aborted && rgb[c[0]] > min[0] && (n < 2 || rgb[c[1]] > min[1]) && (n < 3 || rgb[c[2]] > min[2]))
      {
         var old = deltaE;

         for (var i = 0; i < n; i++) rgb[c[i]]--;

         deltaE = Probe(rgb);
         ReportProbe(c, (int)n, "↓", old, deltaE);

         if (deltaE > old) break;
      }

      while (!ArgyllProbe.Aborted && rgb[c[0]] < max[0] && (n < 2 || rgb[c[1]] < max[1]) && (n < 3 || rgb[c[2]] < max[2]))
      {
         var old = deltaE;

         for (var i = 0; i < n; i++) rgb[c[i]]++;

         deltaE = Probe(rgb);
         ReportProbe(c, (int)n, "↑", old, deltaE);

         if (deltaE <= old) continue;

         for (var i = 0; i < n; i++) rgb[c[i]]--;
         var reverted = deltaE;
         deltaE = Probe(rgb);
         ReportProbe(c, (int)n, "↓", reverted, deltaE, revert: true);
         break;
      }

      return (
          oldGain[0] != rgb[c[0]]
          || oldGain[1] != rgb[c[1]]
          || oldGain[2] != rgb[c[2]]
          );
   }

   static readonly bool PerfTrace =
      Environment.GetEnvironmentVariable("LBM_PERF") is "1" or "true" or "yes";

   double Probe(uint[] rgb)
   {
      var gain = Vcp?.Gain;
      if (gain is null) return 0;

      ref var tune = ref _tune[rgb[0], rgb[1], rgb[2]];
      if (tune != 0) return tune;

      var sw = PerfTrace ? Stopwatch.StartNew() : null;
      if (sw is not null)
         Console.Error.WriteLine(
            $"PERF {DateTime.Now:HH:mm:ss.fff} tune-probe SetTo({rgb[0]},{rgb[1]},{rgb[2]}) queued");

      gain.SetTo(rgb);

      // let the panel settle on the new gains before reading
      Thread.Sleep(SelectedSpeed?.SettleMs ?? 500);

      var settleDone = sw?.ElapsedMilliseconds ?? 0;

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
      else if (!probe.Aborted)
      {
         // a failed reading must not feed ΔE=0 ("perfect") into the optimizer:
         // it would walk the gains into the walls one dead cycle at a time
         probe.Message = "Measurement failed — tuning stopped";
         probe.Abort();
      }

      if (sw is not null)
         Console.Error.WriteLine(
            $"PERF {DateTime.Now:HH:mm:ss.fff} tune-probe done: settle={settleDone} ms, spotread={sw.ElapsedMilliseconds - settleDone} ms, total={sw.ElapsedMilliseconds} ms");

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
      Hisense.Dispose();
      // ends the persistent spotread session along with any running sweep
      ArgyllProbe.Abort();
      Vcp?.Dispose();
   }
}
