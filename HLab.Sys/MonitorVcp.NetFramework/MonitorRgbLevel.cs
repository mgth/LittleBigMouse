using HLab.Notify.PropertyChanged;
using HLab.Windows.Monitors;

namespace HLab.Windows.MonitorVcp
{
    public class MonitorRgbLevel : N<MonitorRgbLevel>
    {
        private readonly MonitorLevel[] _values = new MonitorLevel[3];

        public MonitorRgbLevel(Monitor monitor, LevelParser parser, VcpGetter getter, VcpSetter setter)
        {
            for (uint i = 0; i < 3; i++)
                _values[i] = new MonitorLevel(monitor, parser, getter, setter, i);

        }
        public MonitorLevel Channel(uint channel) { return _values[channel]; }

        public MonitorLevel Red => Channel(0);
        public MonitorLevel Green => Channel(1);
        public MonitorLevel Blue => Channel(2);
    }
}