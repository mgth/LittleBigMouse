using System.Reactive;
using Avalonia;
using Avalonia.Layout;
using Avalonia.Media;
using HLab.Mvvm.ReactiveUI;
using HLab.Sys.Windows.MonitorVcp.Avalonia;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Patterns;

public class TestPatternButtonViewModel : ViewModel<TestPattern>
{
    readonly VcpScreenViewModel _target;

    public TestPatternButtonViewModel(VcpScreenViewModel target)
    {
        _target = target;
        TestPatternCommand = ReactiveCommand.Create<TestPattern>(ShowTestPattern);
    }

    public TestPatternButtonViewModel Set(TestPatternType type)
    {
        TestPatternType = type;
        return this;
    }
    public TestPatternButtonViewModel Set(Color colorA, Color colorB)
    {
        ColorA = colorA;
        ColorB = colorB;
        return this;
    }
    public TestPatternButtonViewModel Set(Orientation orientation)
    {
        Orientation = orientation;
        return this;
    }
    public TestPatternButtonViewModel SetRgb()
    {
        Rgb = true;
        return this;
    }

    public Color ColorA
    {
        get => _colorA;
        set =>this.RaiseAndSetIfChanged(ref _colorA, value);
    }
    Color _colorA = Colors.White;

    public Color ColorB
    {
        get => _colorB;
        set =>this.RaiseAndSetIfChanged(ref _colorB, value);
    }
    Color _colorB = Colors.Black;

    public TestPatternType TestPatternType
    {
        get => _testPatternType;
        set =>this.RaiseAndSetIfChanged(ref  _testPatternType, value);
    }
    TestPatternType _testPatternType = TestPatternType.Solid;

    public bool Rgb
    {
        get => _rgb;
        set => this.RaiseAndSetIfChanged(ref _rgb, value);
    }
    bool _rgb = false;

    public Orientation Orientation
    {
        get => _orientation;
        set =>  this.RaiseAndSetIfChanged(ref _orientation, value);
    }
    Orientation _orientation = Orientation.Horizontal;


    public ReactiveCommand<TestPattern, Unit> TestPatternCommand { get; }

    // TODO : respect mvvm pattern 
    public void ShowTestPattern(TestPattern pattern)
    {
        if (_target.TestPatternPanel == null)
        {
            var x = (_target.Model.ActiveSource.Source.InPixel.X + _target.Model.ActiveSource.Source.InPixel.Center.X) /
                    2;
            var y = (_target.Model.ActiveSource.Source.InPixel.Y + _target.Model.ActiveSource.Source.InPixel.Center.Y) /
                    2;



            var w = pattern.Show(new PixelPoint((int)x, (int)y));

            w.Closing += (o, a) => { _target.TestPatternPanel = null; };
            _target.TestPatternPanel = w;

        }
        else
        {
            if (_target.TestPatternPanel.Content is not TestPattern p) return;

            if (p.PatternColorA == pattern.PatternColorA
                && p.PatternColorB == pattern.PatternColorB
                && p.PatternType == pattern.PatternType
                && p.Rgb == pattern.Rgb
                && p.Orientation == pattern.Orientation
               )
            {
                _target.TestPatternPanel?.Close();
                _target.TestPatternPanel = null;
                return;
            }

            p.PatternColorA = pattern.PatternColorA;
            p.PatternColorB = pattern.PatternColorB;
            p.PatternType = pattern.PatternType;
            p.Rgb = pattern.Rgb;
            p.Orientation = pattern.Orientation;
        }
    }

}