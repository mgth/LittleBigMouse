using System.Windows.Media;
using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.MonitorVcp;

namespace LittleBigMouse.Plugin.Vcp
{
    using H = H<TestViewModel>;

    internal class TestViewModel : NotifierBase
    {
        public TestViewModel() => H.Initialize(this);
        public TestPatternType TestType => _testType.Get();
        readonly IProperty<TestPatternType> _testType = H.Property<TestPatternType>();
        public Color TestColor => _testColor.Get();
        readonly IProperty<Color> _testColor = H.Property<Color>();
    }
}
