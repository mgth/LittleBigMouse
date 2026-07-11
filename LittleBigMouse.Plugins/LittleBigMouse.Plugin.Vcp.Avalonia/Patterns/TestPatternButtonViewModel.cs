using System.Reactive;
using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Platform;
using Avalonia.Layout;
using Avalonia.Media;
using HLab.Mvvm.ReactiveUI;
using HLab.Sys.Windows.MonitorVcp.Avalonia;
using LittleBigMouse.Plugins.Avalonia;
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

    static bool SamePattern(TestPattern a, TestPattern b)
        => a.PatternColorA == b.PatternColorA
           && a.PatternColorB == b.PatternColorB
           && a.PatternType == b.PatternType
           && a.Rgb == b.Rgb
           && a.Orientation == b.Orientation;

    /// <summary>
    /// Native-Wayland path: render the pattern at the output's native
    /// resolution and hand it to the lbm-pattern helper — the only way to get
    /// pixel-perfect output on every screen of a mixed-scale desktop (the
    /// Avalonia window goes through XWayland and gets rescaled).
    /// Returns false to fall back to the Avalonia window.
    /// </summary>
    bool ShowNativePattern(TestPattern pattern)
    {
        var connector = _target.Model.DeviceId;
        if (string.IsNullOrEmpty(connector)) return false;

        if (_target.NativePatternProcess is not null)
        {
            var shown = _target.NativeShownPattern;
            _target.CloseNativePattern();
            // same button again = toggle off
            if (shown is not null && SamePattern(shown, pattern)) return true;
        }

        if (!WaylandPattern.ListOutputs().TryGetValue(connector, out var size)) return false;

        var pngPath = System.IO.Path.Combine(System.IO.Path.GetTempPath(),
            $"lbm-pattern-{Environment.ProcessId}-{connector}.png");

        var rendered = pattern.Clone();
        rendered.ChessCell = 1; // the buffer reaches the panel 1:1
        rendered.Measure(new Size(size.Width, size.Height));
        rendered.Arrange(new Rect(0, 0, size.Width, size.Height));
        using (var bitmap = new global::Avalonia.Media.Imaging.RenderTargetBitmap(
                   new PixelSize(size.Width, size.Height), new Vector(96, 96)))
        {
            bitmap.Render(rendered);
            bitmap.Save(pngPath);
        }

        var process = WaylandPattern.Show(connector, pngPath);
        if (process is null) return false;

        _target.NativePatternProcess = process;
        _target.NativeShownPattern = pattern.Clone();

        process.EnableRaisingEvents = true;
        process.Exited += (_, _) =>
        {
            // clicked away (the helper exits on click/key) — drop stale state
            if (ReferenceEquals(_target.NativePatternProcess, process))
            {
                _target.NativePatternProcess = null;
                _target.NativeShownPattern = null;
            }
            try { System.IO.File.Delete(pngPath); } catch (Exception) { }
        };

        return true;
    }

    // TODO : respect mvvm pattern
    public void ShowTestPattern(TestPattern pattern)
    {
        if (WaylandPattern.IsAvailable && ShowNativePattern(pattern)) return;

        if (_target.TestPatternPanel == null)
        {
            var source = _target.Model.ActiveSource.Source;
            var inPixel = source.InPixel;

            // The layout space and the windowing-system space may differ
            // (KWin/XWayland global factor): aim at the matching Avalonia
            // screen, not at the layout coordinates.
            Screen? matched = null;
            var w = pattern.Show(screens =>
            {
                matched = ScreenFinder.FromLayoutBounds(screens, inPixel.Bounds);

                return matched is not null
                    ? new PixelPoint(
                        matched.Bounds.X + matched.Bounds.Width / 2,
                        matched.Bounds.Y + matched.Bounds.Height / 2)
                    : new PixelPoint(
                        (int)((inPixel.X + inPixel.Center.X) / 2),
                        (int)((inPixel.Y + inPixel.Center.Y) / 2));
            });

            // Gamma checker cells: 1 buffer pixel when the buffer reaches the
            // panel 1:1, else just big enough to survive the compositor's
            // resample (physical px per buffer px = monitor scale / global
            // XWayland factor). Windows composites 1:1, no correction.
            if (w.Content is TestPattern shown)
            {
                var cell = 1;
                if (!OperatingSystem.IsWindows() && matched is not null && inPixel.Width > 0)
                {
                    var k = matched.Bounds.Width / inPixel.Width;
                    var scale = source.EffectiveDpi.X / 96.0;
                    var ratio = scale / k;
                    if (ratio < 0.98) cell = Math.Max(2, (int)Math.Ceiling(2.0 / ratio));
                }
                shown.ChessCell = cell;
            }

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