using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using HLab.Mvvm;
using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.MonitorVcp;

namespace LittleBigMouse.Plugin.Vcp.Patterns
{
    using H = H<TestPatternButtonViewModel>;

    class TestPatternButtonViewModel : ViewModel<TestPattern>
    {
        private readonly VcpScreenViewModel _target;


        public TestPatternButtonViewModel(VcpScreenViewModel target)
        {
            _target = target;
            H.Initialize(this);
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
            get => _colorA.Get();
            set => _colorA.Set(value);
        }
        private readonly IProperty<Color> _colorA = H.Property<Color>(c => c.Default(Colors.White));

        public Color ColorB
        {
            get => _colorB.Get();
            set => _colorB.Set(value);
        }
        private readonly IProperty<Color> _colorB = H.Property<Color>(c => c.Default(Colors.Black));

        public TestPatternType TestPatternType
        {
            get => _testPatternType.Get();
            set => _testPatternType.Set(value);
        }
        private readonly IProperty<TestPatternType> _testPatternType = H.Property<TestPatternType>(c => c.Default(TestPatternType.Solid));

        public bool Rgb
        {
            get => _rgb.Get();
            set => _rgb.Set(value);
        }
        private readonly IProperty<bool> _rgb = H.Property<bool>(c => c.Default(false));

        public Orientation Orientation
        {
            get => _orientation.Get();
            set => _orientation.Set(value);
        }
        private readonly IProperty<Orientation> _orientation = H.Property<Orientation>(c => c.Default(Orientation.Horizontal));


        public ICommand TestPatternCommand { get; } = H.Command(c => c
            .Action((e,p) => e.ShowTestPattern(p as TestPattern))
        );

        public void ShowTestPattern(TestPattern pattern)
        {


            if (_target.TestPatternPanel != null)
            {

                if (_target.TestPatternPanel.Content is TestPattern p)
                {
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
                    else
                    {
                        p.PatternColorA = pattern.PatternColorA;
                        p.PatternColorB = pattern.PatternColorB;
                        p.PatternType = pattern.PatternType;
                        p.Rgb = pattern.Rgb;
                        p.Orientation = pattern.Orientation;
                    }
                }
            }
            else
            {
                var x = (_target.Model.InDip.X + _target.Model.InDip.Center.X) / 2;
                var y = (_target.Model.InDip.Y + _target.Model.InDip.Center.Y) / 2;

                var area = new Rect(new Point(x,y),new Size(1,1));
                _target.TestPatternPanel = pattern.Show(area);
            }
        }

    }
}
