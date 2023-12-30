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
using System.Reactive.Linq;
using System.Windows.Input;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.ReactiveUI;
using HLab.Sys.Argyll;
using HLab.Sys.Windows.Monitors;
using HLab.Sys.Windows.MonitorVcp;
using HLab.Sys.Windows.MonitorVcp.Avalonia;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugin.Vcp.Avalonia.Patterns;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

public class VcpScreenViewModelDesign()
    : VcpScreenViewModel(vm => new TestPatternButtonViewModel(vm), null), IDesignViewModel;

public class VcpScreenViewModel : ViewModel<PhysicalMonitor>
{
    readonly ISystemMonitorsService _monitorsService;

    // TODO : use reactive ui for collections
    public VcpScreenViewModel(
        Func<VcpScreenViewModel, TestPatternButtonViewModel> getButtonPattern, 
        ISystemMonitorsService monitorsService)
    {
        _monitorsService = monitorsService;

        TestPatterns.Add(getButtonPattern(this).Set(TestPatternType.Circles).Set(Colors.White, Colors.Black));
        TestPatterns.Add(getButtonPattern(this).Set(TestPatternType.Circle).Set(Color.FromRgb(0xFF, 0x80, 0x00), Colors.Black));
        TestPatterns.Add(getButtonPattern(this).Set(TestPatternType.Gradient).SetRgb());
        TestPatterns.Add(getButtonPattern(this)
            .Set(TestPatternType.Gamma)
            .Set(Colors.White, Colors.Black)
            .Set(Orientation.Vertical).SetRgb());

        _vcp = this.WhenAnyValue(
                e => e.Model,
                selector: e => e?.MonitorDevice(_monitorsService).Vcp().Start())
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

       _anywayVisibility = this.WhenAnyValue(
            e => e.Vcp.Brightness,
            e => e.Vcp.Contrast,
            (b,c) => b == null || c == null)
            .ToProperty(this, e => e.AnywayVisibility);

       this.WhenAnyValue(e => e.Model).Do(e => InitLut()).Subscribe();

       AnywayCommand = ReactiveCommand.Create(() => Vcp?.ActivateAnyway());

       SwitchSourceCommand = ReactiveCommand.Create(SwitchSource);
    }

    public bool BrightnessVisibility => _brightnessVisibility.Value;
    readonly ObservableAsPropertyHelper<bool> _brightnessVisibility;

    public bool ContrastVisibility => _contrastVisibility.Value;
    readonly ObservableAsPropertyHelper<bool> _contrastVisibility ;

    public bool GainVisibility => _gainVisibility.Value;
    readonly ObservableAsPropertyHelper<bool> _gainVisibility;

    public bool DriveVisibility => _driveVisibility.Value;
    readonly ObservableAsPropertyHelper<bool> _driveVisibility;

    public bool AnywayVisibility => _anywayVisibility.Value;
    readonly ObservableAsPropertyHelper<bool> _anywayVisibility;

    public ICommand AnywayCommand { get; }

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

    public ObservableCollection<TestPatternButtonViewModel> TestPatterns { get; } = new();

    public ProbeLut? Lut => Model?.MonitorDevice(_monitorsService).ProbeLut();

    void InitLut()
    {
        Lut?.Load();
    }


    public void ProbeLowLuminance()
    {
        var probe = new ArgyllProbe();
        if (!probe.Installed)
        {
            PleaseInstall();
            return;
        }


        new Thread(() =>
        {
            var level = Vcp.Brightness;

            var max = Vcp.Gain.Red.Max;
            var min = Vcp.Gain.Red.Min;

            var old = Model.MonitorDevice(_monitorsService).ProbeLut().Luminance;
            level.Value = 0;

            for (var i = max; i >= min; i--)
            {


                TuneWhitePoint(i);

                var t = Lut.Current;

                if (probe.SpotRead())
                {
                    t.Y = probe.ProbedColor.xyY.Y;
                    t.x = probe.ProbedColor.xyY.x;
                    t.y = probe.ProbedColor.xyY.y;
                }

                Lut.RemoveLowBrightness(i);
                Lut.Add(t);

                    //_line.Points.Add(new DataPoint(i, t.Y));
                    //Curve.Refresh();

                    if (t.MinGain <= min) break;
            }

            Model.MonitorDevice(_monitorsService).ProbeLut().Luminance = old;
        }
        ).Start();
        return;
    }

    void ProbeLuminance()
    {
        var probe = new ArgyllProbe();
        if (!probe.Installed)
        {
            PleaseInstall();
            return;
        }


        Task.Run(() =>
        {
            var level = Vcp.Brightness;
 
            for (var i = level.Min; i <= level.Max; i++)
            {
                level.Value = i;

                TuneWhitePoint();

                var t = Lut.Current;
                if (probe.SpotRead())
                {
                    t.Y = probe.ProbedColor.xyY.Y;
                    t.x = probe.ProbedColor.xyY.x;
                    t.y = probe.ProbedColor.xyY.y;
                }

                Lut.RemoveBrightness(t.Brightness);
                Lut.Add(t);

            }
        }
        );
        return;

        new Thread(() =>
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
                    level.Value = i; Thread.Sleep(1000);
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
        }
        ).Start();
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

        new Thread(() =>
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
                    level.Value = i; Thread.Sleep(1000);

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
        }
        ).Start();
    }

    double[,,] _tune = new double[0,0,0];
    //private LineSeries _line;

    public void Tune()
    {
        new Thread(TuneWhitePoint).Start();
    }


    public void TuneWhitePoint()
    {
        TuneWhitePoint(0);
    }
    public void TuneWhitePoint(uint max)
    {
        //    _line = new LineSeries();
        //    Curve.PlotModel.Series.Add(_line);

        var count = 6;
        uint channel = 0;

        _tune = new double[Vcp.Gain.Red.Max + 1, Vcp.Gain.Green.Max + 1, Vcp.Gain.Blue.Max + 1];

        uint[] rgb = { Vcp.Gain.Red.Value, Vcp.Gain.Green.Value, Vcp.Gain.Blue.Value };

        while (count > 0)
        {
            if (TuneWhitePoint(channel, ref rgb, max)) count = 5;
            else count--;

            channel++;
            channel %= 6;
        }

        Vcp.Gain.Red.Value = rgb[0];
        Vcp.Gain.Green.Value = rgb[1];
        Vcp.Gain.Blue.Value = rgb[2];
    }

    bool TuneWhitePoint(uint channel, ref uint[] rgb, uint maxlevel)
    {
        uint[] c;
        var min = new uint[3];
        var max = new uint[3];
        uint n = 0;

        switch (channel)
        {
            case 0:
                c = new uint[] { 0, 1, 2 };
                n = 1;
                break;
            case 1:
                c = new uint[] { 1, 2, 0 };
                n = 1;
                break;
            case 2:
                c = new uint[] { 2, 0, 1 };
                n = 1;
                break;
            case 3: //0
                c = new uint[] { 0, 1, 2 };
                n = 2;
                break;
            case 4: //1
                c = new uint[] { 1, 2, 0 };
                n = 2;
                break;
            case 5: //2
                c = new uint[] { 2, 0, 1 };
                n = 2;
                break;
            default:
                c = new uint[] { 0, 1, 2 };
                n = 3;
                break;
        }


        for (var i = 0; i < 3; i++)
        {
            min[i] = Vcp.Gain.Channel(c[i]).Min;
            max[i] = maxlevel == 0 ? Vcp.Gain.Channel(c[i]).Max : maxlevel;
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



        uint[] oldGain = { rgb[c[0]], rgb[c[1]], rgb[c[2]] };
        //uint old2 = rgb[c2];

        var deltaE = Probe(rgb);
        //_line.Points.Add(new DataPoint(_line.Points.Count, deltaE));
        //Curve.Refresh();

        while (rgb[c[0]] > min[0] && (n < 2 || rgb[c[1]] > min[1]) && (n < 3 || rgb[c[2]] > min[2]))
        {
            var old = deltaE;

            for (var i = 0; i < n; i++) rgb[c[i]]--;

            deltaE = Probe(rgb);

            if (deltaE > old) break;

            //_line.Points.Add(new DataPoint(_line.Points.Count,deltaE));
            //Curve.Refresh();
        }

        while (rgb[c[0]] < max[0] && (n < 2 || rgb[c[1]] < max[1]) && (n < 3 || rgb[c[2]] < max[2]))
        {
            var old = deltaE;

            for (var i = 0; i < n; i++) rgb[c[i]]++;

            deltaE = Probe(rgb);

            if (deltaE > old)
            {
                for (var i = 0; i < n; i++) rgb[c[i]]--;
                deltaE = Probe(rgb);
                break;
            }

            //_line.Points.Add(new DataPoint(_line.Points.Count, deltaE));
            //Curve.Refresh();
        }

        return (
            oldGain[0] != rgb[c[0]]
            || oldGain[1] != rgb[c[1]]
            || oldGain[2] != rgb[c[2]]
            );
    }

    double Probe(uint[] rgb)
    {
        if (_tune[rgb[0], rgb[1], rgb[2]] == 0)
        {
            Vcp.Gain.Red.Value = rgb[0];
            Vcp.Gain.Green.Value = rgb[1];
            Vcp.Gain.Blue.Value = rgb[2];

            Thread.Sleep(500);

            var probe = new ArgyllProbe(true);
            if (!probe.Installed)
            {
                PleaseInstall();
                return 0;
            }
            if (probe.SpotRead())
            {
                _tune[rgb[0], rgb[1], rgb[2]] = probe.ProbedColor.DeltaE00();
            }

            //return color.DeltaE00();
        }
        return _tune[rgb[0], rgb[1], rgb[2]];
    }
    public void Save()
    {
        Lut.Save();
    }

    static void PleaseInstall()
    {
        // MessageBox.Show("Please install DispcalGUI & ArgyllCMS", "Calibration tools",
        //              MessageBoxButton.OK, MessageBoxImage.Exclamation);
    }


    public ICommand SwitchSourceCommand { get; }

    void SwitchSource()
    {
        Vcp.SetSource(12);
    }

    public override void OnDispose()
    {
        VcpExpendMonitor.Stop();
    }
}
