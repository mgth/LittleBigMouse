using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Media;
using HLab.Notify.PropertyChanged;
using HLab.Windows.MonitorVcp;

namespace LittleBigMouse.Plugin.Vcp
{
    class TestViewModel : N<TestViewModel>
    {
        public TestPatternType TestType => _testType.Get();
        private readonly IProperty<TestPatternType> _testType = H.Property<TestPatternType>();
        public Color TestColor => _testColor.Get();
        private readonly IProperty<Color> _testColor = H.Property<Color>();
    }
}
