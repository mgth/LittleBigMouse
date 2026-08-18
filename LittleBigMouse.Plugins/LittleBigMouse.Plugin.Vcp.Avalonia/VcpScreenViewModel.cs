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
using HLab.Base.ReactiveUI;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.ReactiveUI;
using HLab.Sys.Argyll;
using HLab.Sys.Windows.MonitorVcp;
using HLab.Sys.Windows.MonitorVcp.Avalonia;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugin.Vcp.Avalonia.Calibration;
using LittleBigMouse.Plugin.Vcp.Avalonia.Patterns;
using LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;
using LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;
using LittleBigMouse.Plugin.Vcp.Calibration;
using LiveChartsCore;
using LiveChartsCore.Kernel;
using LiveChartsCore.SkiaSharpView;
using LiveChartsCore.SkiaSharpView.Painting;
using LiveChartsCore.SkiaSharpView.Painting.Effects;
using ReactiveUI;
using SkiaSharp;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

public class VcpScreenViewModelDesign()
    : VcpScreenViewModel(vm => new TestPatternButtonViewModel(vm), null, null, null, null, null), IDesignViewModel;

public record ObserverChoice(ArgyllProbe.ObserverEnum Value, string Label);

public record SpeedChoice(string Label, bool Adaptive, int SettleMs);

record VcpResolution(VcpControl? Control, bool Resolving);

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
   readonly ICalibrationService _calibrationService;
   readonly CalibrationTaskCoordinator _calibrations;
   static readonly SemaphoreSlim SpotreadCommandGate = new(1, 1);
   int _disposed;

   // TODO : use reactive ui for collections
   public VcpScreenViewModel(
       Func<VcpScreenViewModel, TestPatternButtonViewModel> getButtonPattern,
       IVcpService? vcpService,
       ISamsungTizenService? samsungTizenService,
       IHisenseVidaaService? hisenseVidaaService,
       ILayoutOptions? layoutOptions,
       ICalibrationService? calibrationService)
   {
      _vcpService = vcpService;
      _calibrationService = calibrationService ?? new CalibrationService();
      _calibrations = new CalibrationTaskCoordinator(ReportCalibrationError);

      // experimental gate: Argyll calibration and the smart-TV test tooling
      // stay hidden unless enabled in the application options
      _experimentalEnabled = (layoutOptions is null
              ? Observable.Return(false)
              : layoutOptions.WhenAnyValue(o => o.ExperimentalFeatures))
          .ToProperty(this, e => e.ExperimentalEnabled)
          .DisposeWith(this);
      Samsung = new SamsungControlViewModel(samsungTizenService);
      Hisense = new HisenseControlViewModel(hisenseVidaaService);

      this.WhenAnyValue(e => e.Model)
          .Subscribe(Samsung.SetMonitor)
          .DisposeWith(this);
      this.WhenAnyValue(e => e.Model)
          .Subscribe(Hisense.SetMonitor)
          .DisposeWith(this);

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
      var vcpResolution = this.WhenAnyValue(e => e.Model)
          .Select(m => m is null || _vcpService is null
              ? Observable.Return(new VcpResolution(null, false))
              : Observable.Concat(
                  Observable.Return(new VcpResolution(null, true)),
                  Observable.FromAsync(ct => _vcpService.GetControlAsync(m, ct))
                      .Select(control => new VcpResolution(control, false))
                      .Catch<VcpResolution, Exception>(error =>
                      {
                         Console.Error.WriteLine($"VCP: unable to resolve {m.Id}: {error.Message}");
                         return Observable.Return(new VcpResolution(null, false));
                      })))
          .Switch()
          .ObserveOn(RxSchedulers.MainThreadScheduler)
          .Replay(1)
          .RefCount();

      _vcp = vcpResolution
          .Select(resolution => resolution.Control?.Start())
          .ToProperty(this, e => e.Vcp)
          .DisposeWith(this);

      _vcpResolving = vcpResolution
          .Select(resolution => resolution.Resolving)
          .ToProperty(this, e => e.VcpResolving)
          .DisposeWith(this);

      _brightnessVisibility = this.WhenAnyValue(
          e => e.Vcp.Brightness,
          selector: e => e != null)
          .ToProperty(this, e => e.BrightnessVisibility)
          .DisposeWith(this);

      _contrastVisibility = this.WhenAnyValue(
          e => e.Vcp.Contrast,
          selector: e => e != null)
          .ToProperty(this, e => e.ContrastVisibility)
          .DisposeWith(this);

      _gainVisibility = this.WhenAnyValue(
           e => e.Vcp.Gain,
           selector: e => e != null)
           .ToProperty(this, e => e.GainVisibility)
           .DisposeWith(this);

      _driveVisibility = this.WhenAnyValue(
           e => e.Vcp.Drive,
           selector: e => e != null)
           .ToProperty(this, e => e.DriveVisibility)
           .DisposeWith(this);

      // hidden while the capabilities probe is still running: no point offering
      // the forced activation before knowing what the monitor answered
      _anywayVisibility = this.WhenAnyValue(
           e => e.Vcp.Brightness,
           e => e.Vcp.Contrast,
           e => e.Vcp.Probing,
           e => e.VcpResolving,
           (b, c, probing, resolving) => !resolving && !probing && (b == null || c == null))
           .ToProperty(this, e => e.AnywayVisibility)
           .DisposeWith(this);

      _imageVisibility = this.WhenAnyValue(
           e => e.Vcp.Brightness,
           e => e.Vcp.Contrast,
           (b, c) => b != null || c != null)
           .ToProperty(this, e => e.ImageVisibility)
           .DisposeWith(this);

      // the advanced section is only meaningful once the monitor answered with
      // at least one adjustable level; without the experimental gate it only
      // holds the drive faders, calibration being hidden
      _advancedVisibility = this.WhenAnyValue(
           e => e.Vcp.Brightness,
           e => e.Vcp.Contrast,
           e => e.Vcp.Drive,
           e => e.ExperimentalEnabled,
           (b, c, d, experimental) => d != null || (experimental && (b != null || c != null)))
           .ToProperty(this, e => e.AdvancedVisibility)
           .DisposeWith(this);

      _calibrationVisibility = this.WhenAnyValue(
           e => e.BrightnessVisibility,
           e => e.ExperimentalEnabled,
           (brightness, experimental) => brightness && experimental)
           .ToProperty(this, e => e.CalibrationVisibility)
           .DisposeWith(this);

      _probingVisibility = this.WhenAnyValue(
           e => e.Vcp.Probing,
           e => e.VcpResolving,
           (probing, resolving) => probing || resolving)
           .ToProperty(this, e => e.ProbingVisibility)
           .DisposeWith(this);

      _message = this.WhenAnyValue(
           e => e.Vcp.Probing,
           e => e.VcpResolving,
           (probing, resolving) => resolving
               ? "Locating this monitor's DDC/CI channel…"
               : probing
                   ? "Reading this monitor's available controls…"
                   : "")
           .ToProperty(this, e => e.Message)
           .DisposeWith(this);

      _selectedSpeed = Speeds[1];

      // the speed preset drives spotread's adaptive integration flag
      this.WhenAnyValue(e => e.SelectedSpeed)
          .Subscribe(s => { if (s is not null) ArgyllProbe.Adaptive = s.Adaptive; })
          .DisposeWith(this);

      // persisted calibration settings: common file, overridden per monitor
      this.WhenAnyValue(e => e.Model)
          .Subscribe(m => { if (m is not null) LoadCalibrationSettings(m.Id); })
          .DisposeWith(this);

      this.WhenAnyValue(
           e => e.ArgyllProbe.ColorTemp,
           e => e.ArgyllProbe.Observer,
           e => e.SelectedSpeed)
          .Subscribe(_ => SaveCalibrationValues())
          .DisposeWith(this);

      this.WhenAnyValue(e => e.TestPairs)
          .Subscribe(_ => SaveMonitorCalibration())
          .DisposeWith(this);

      this.WhenAnyValue(e => e.UseCustomSettings)
          .Subscribe(OnUseCustomChanged)
          .DisposeWith(this);

      // what will actually be launched, kept honest by recomputing from the probe
      _spotreadCommand = this.WhenAnyValue(
           e => e.ArgyllProbe.Observer,
           e => e.ArgyllProbe.ColorTemp,
           e => e.SelectedSpeed,
           e => e.ExperimentalEnabled)
           .Select(values => values.Item4
               ? Observable.FromAsync(LoadSpotreadCommandAsync)
                   .StartWith("Detecting ArgyllCMS…")
               : Observable.Return(""))
           .Switch()
           .ObserveOn(RxSchedulers.MainThreadScheduler)
           .ToProperty(this, e => e.SpotreadCommand)
           .DisposeWith(this);


      AnywayCommand = ReactiveCommand.Create(() => Vcp?.ActivateAnyway())
          .DisposeWith(this);

      var probeBrightnessCommand = ReactiveCommand.CreateFromTask(() => ProbeBrightnessAsync());
      probeBrightnessCommand.ThrownExceptions
          .Subscribe(_ => { })
          .DisposeWith(this);
      ProbeBrightnessCommand = probeBrightnessCommand.DisposeWith(this);

      ClearLutCommand = ReactiveCommand.Create(() =>
      {
         var lut = ProbeLut;
         if (lut is null) return;
         lut.Clear();
         lut.Save();
         LastMeasure = "";
         ProbeLog.Clear();
         ProbeVisible = false;
      }).DisposeWith(this);

      StopTuneCommand = ReactiveCommand.Create(() =>
      {
         _calibrations.Cancel();
         ArgyllProbe.Abort();
         ArgyllProbe.Message = "Stopping…";
      }).DisposeWith(this);

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
          .ToProperty(this, e => e.ProbeLut)
          .DisposeWith(this);


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
            .ToProperty(this, e => e.Series)
            .DisposeWith(this);

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
            .ToProperty(this, e => e.XAxes)
            .DisposeWith(this);

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
            .ToProperty(this, e => e.YAxes)
            .DisposeWith(this);
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
   void ReportProbe(WhitePointAdjustment adjustment)
   {
      var r = adjustment.Channels.HasFlag(CalibrationChannels.Red);
      var g = adjustment.Channels.HasFlag(CalibrationChannels.Green);
      var b = adjustment.Channels.HasFlag(CalibrationChannels.Blue);
      var arrow = adjustment.Direction switch
      {
         CalibrationDirection.Down => "↓",
         CalibrationDirection.Up => "↑",
         CalibrationDirection.Revert => "↓",
         _ => "",
      };
      var previous = adjustment.PreviousDeltaE;
      var current = adjustment.CurrentDeltaE;

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
         (verdict, brush) = adjustment.Direction == CalibrationDirection.Revert
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
      ScheduleOnUi(() =>
      {
         ProbeLog.Insert(0, entry);
         while (ProbeLog.Count > ProbeLogLength) ProbeLog.RemoveAt(ProbeLog.Count - 1);
         ProbeVisible = true;
      });
   }

   public VcpControl? Vcp => _vcp.Value;
   readonly ObservableAsPropertyHelper<VcpControl?> _vcp;

   public bool VcpResolving => _vcpResolving.Value;
   readonly ObservableAsPropertyHelper<bool> _vcpResolving;

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

   async Task<string> LoadSpotreadCommandAsync(CancellationToken cancellationToken)
   {
      await SpotreadCommandGate.WaitAsync(cancellationToken).ConfigureAwait(false);
      try
      {
         return await Task.Run(() =>
         {
            cancellationToken.ThrowIfCancellationRequested();
            var command = $"spotread{ArgyllProbe.SpotReadArgs}   →   D{ArgyllProbe.ColorTemp / 100:0} target";
            cancellationToken.ThrowIfCancellationRequested();
            return command;
         }, cancellationToken).ConfigureAwait(false);
      }
      finally
      {
         SpotreadCommandGate.Release();
      }
   }

   public Task ProbeLowLuminance(CancellationToken cancellationToken = default)
   {
      if (!ArgyllProbe.Installed)
      {
         PleaseInstall();
         return Task.CompletedTask;
      }

      var control = Vcp;
      var lut = ProbeLut;
      if (control?.Brightness is null || control.Gain is null || lut is null)
         return Task.CompletedTask;

      var input = new LowLuminanceCalibrationInput(WhitePointInput());
      return RunCalibrationAsync(
          "low-luminance probe",
          ArgyllProbe,
          control,
          async (hardware, progress, token) =>
          {
             var result = await _calibrationService.CalibrateLowLuminanceAsync(
                 hardware, input, progress, token).ConfigureAwait(false);
             SetCompletionMessage(result.Completion);
          },
          progress => ProjectLowLuminanceProgress(lut, progress),
          cancellationToken);
   }

   async Task ProbeBrightnessAsync(CancellationToken cancellationToken = default)
   {
      var control = Vcp;
      var lut = ProbeLut;
      if (control?.Brightness is null || control.Gain is null || lut is null) return;

      // the panel's probe instance: its Message property is bound in the
      // calibration section, so progress and errors are actually visible
      var probe = ArgyllProbe;
      if (!probe.Installed)
      {
         probe.Message = "ArgyllCMS (spotread) not found — install argyllcms";
         return;
      }

      var measured = lut.SortedLut
          .Select(t => (uint)t.Brightness)
          .ToHashSet();
      var input = new BrightnessCalibrationInput(
          control.Brightness.Min,
          control.Brightness.Max,
          measured,
          WhitePointInput());

      await RunCalibrationAsync(
          "brightness probe",
          probe,
          control,
          async (hardware, progress, token) =>
          {
             var result = await _calibrationService.CalibrateBrightnessAsync(
                 hardware, input, progress, token).ConfigureAwait(false);
             SetCompletionMessage(result.Completion);
          },
          progress => ProjectBrightnessProgress(lut, progress),
          cancellationToken,
          showRunning: true).ConfigureAwait(false);
   }

   public Task Probe(CancellationToken cancellationToken = default)
   {
      var control = Vcp;
      if (control?.Gain is null || control.Contrast is null) return Task.CompletedTask;

      var probe = new ArgyllProbe();
      if (!probe.Installed)
      {
         PleaseInstall();
         return Task.CompletedTask;
      }

      var input = new ContrastCalibrationInput(control.Contrast.Min, control.Contrast.Max);
      return RunCalibrationAsync(
          "contrast probe",
          probe,
          control,
          async (hardware, progress, token) =>
          {
             await _calibrationService.ProbeContrastAsync(
                 hardware, input, progress, token).ConfigureAwait(false);
          },
          ProjectCalibrationProgress,
          cancellationToken);
   }

   public Task Tune(CancellationToken cancellationToken = default)
   {
      var control = Vcp;
      if (control?.Gain is null) return Task.CompletedTask;

      return RunCalibrationAsync(
          "white-point tuning",
          ArgyllProbe,
          control,
          async (hardware, progress, token) =>
          {
             var result = await _calibrationService.TuneWhitePointAsync(
                 hardware, WhitePointInput(), progress, token).ConfigureAwait(false);
             SetCompletionMessage(result.Completion);
          },
          ProjectCalibrationProgress,
          cancellationToken);
   }

   WhitePointCalibrationInput WhitePointInput(uint maximumGain = 0)
      => new(
          maximumGain,
          TestPairs,
          TimeSpan.FromMilliseconds(SelectedSpeed?.SettleMs ?? 500));

   void ProjectCalibrationProgress(CalibrationProgress progress)
   {
      if (progress.Adjustment is { } adjustment)
      {
         ReportProbe(adjustment);
         return;
      }

      var message = progress.Stage switch
      {
         CalibrationStage.Initializing => "Initializing instrument — calibration may be requested…",
         CalibrationStage.SettingBrightness => $"Measuring brightness {progress.Current} / {progress.Total}…",
         CalibrationStage.SettingContrast => $"Measuring contrast {progress.Current} / {progress.Total}…",
         CalibrationStage.Measuring => "Measuring…",
         _ => null,
      };
      if (message is not null) ScheduleOnUi(() => ArgyllProbe.Message = message);
   }

   void ProjectBrightnessProgress(ProbeLut lut, CalibrationProgress progress)
   {
      ProjectCalibrationProgress(progress);
      if (progress.Point is not { } point) return;

      ScheduleOnUi(() =>
      {
         var tune = ToTune(point);
         lut.RemoveBrightness(tune.Brightness);
         lut.Add(tune);
         lut.Save();
         LastMeasure = $"B {tune.Brightness:0} → {tune.Y:0.0} cd/m² · R {tune.Red:0} G {tune.Green:0} B {tune.Blue:0} · ΔE00 {tune.DeltaE:0.00}";
      });
   }

   void ProjectLowLuminanceProgress(ProbeLut lut, CalibrationProgress progress)
   {
      ProjectCalibrationProgress(progress);
      if (progress.Point is not { } point) return;

      ScheduleOnUi(() =>
      {
         var tune = ToTune(point);
         lut.RemoveLowBrightness(tune.MaxGain);
         lut.Add(tune);
         lut.Save();
      });
   }

   static Tune ToTune(CalibrationPoint point) => new()
   {
      Date = DateTime.Now,
      Brightness = point.Display.Brightness,
      Contrast = point.Display.Contrast,
      Red = point.Display.Gains.Red,
      Green = point.Display.Gains.Green,
      Blue = point.Display.Gains.Blue,
      Y = point.Measurement.Luminance,
      x = point.Measurement.ChromaticityX,
      y = point.Measurement.ChromaticityY,
      DeltaE = point.Measurement.DeltaE,
   };

   void SetCompletionMessage(CalibrationCompletion completion)
      => ScheduleOnUi(() => ArgyllProbe.Message = completion == CalibrationCompletion.Converged
          ? "White point tuning done"
          : "White point tuning did not converge");

   async Task RunCalibrationAsync(
       string operationName,
       ArgyllProbe probe,
       VcpControl control,
       Func<ICalibrationHardware, IProgress<CalibrationProgress>, CancellationToken, Task> operation,
       Action<CalibrationProgress> projectProgress,
       CancellationToken cancellationToken,
       bool showRunning = false)
   {
      var hardware = new VcpCalibrationHardware(control, probe);
      var progress = new CallbackProgress<CalibrationProgress>(projectProgress);
      await _calibrations.RunAsync(
          operationName,
          async token =>
          {
             probe.ResetAbort();
             using var abortRegistration = token.Register(probe.Abort);
             token.ThrowIfCancellationRequested();
             if (showRunning)
             {
                ScheduleOnUi(() =>
                {
                   TuneRunning = true;
                   probe.Message = "Initializing instrument — calibration may be requested…";
                });
             }

             try
             {
                await operation(hardware, progress, token).ConfigureAwait(false);
             }
             finally
             {
                if (token.IsCancellationRequested)
                   ScheduleOnUi(() => probe.Message = "Tuning stopped");
                if (showRunning) ScheduleOnUi(() => TuneRunning = false);
             }
          },
          cancellationToken).ConfigureAwait(false);
   }

   sealed class CallbackProgress<T>(Action<T> callback) : IProgress<T>
   {
      public void Report(T value) => callback(value);
   }

   void ReportCalibrationError(string operationName, Exception error)
   {
      Console.Error.WriteLine($"VCP calibration '{operationName}' failed: {error}");
      ScheduleOnUi(() => ArgyllProbe.Message = $"Calibration failed: {error.Message}");
   }

   void ScheduleOnUi(Action action)
   {
      RxSchedulers.MainThreadScheduler.Schedule(() =>
      {
         if (Volatile.Read(ref _disposed) == 0) action();
      });
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
      if (Interlocked.Exchange(ref _disposed, 1) != 0) return;

      CloseNativePattern();
      Hisense.Dispose();
      // Abort releases a blocking spotread call. The coordinator cancellation
      // stops delays and loops, then disposes the DDC control only after the
      // calibration code has relinquished it.
      ArgyllProbe.Abort();
      var control = Vcp;
      _calibrations.Dispose(() => control?.Dispose());
   }
}
