using System.Windows.Media;
using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.MonitorVcp;

namespace LittleBigMouse.Plugin.Vcp
{
    using H = H<TestViewModel>;

    class TestViewModel : NotifierBase
    {
        public TestViewModel() => H.Initialize(this);
        public TestPatternType TestType => _testType.Get();
        private readonly IProperty<TestPatternType> _testType = H.Property<TestPatternType>();
        public Color TestColor => _testColor.Get();
        private readonly IProperty<Color> _testColor = H.Property<Color>();
    }
}
