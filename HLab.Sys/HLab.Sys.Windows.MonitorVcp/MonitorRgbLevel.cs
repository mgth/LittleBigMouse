using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.Monitors;
using HLab.Sys.Windows.MonitorVcp;

namespace HLab.Sys.Windows.MonitorVcp
{
    using H = H<MonitorRgbLevel>;

    public class MonitorRgbLevel : NotifierBase
    {
        private readonly MonitorLevel[] _values = new MonitorLevel[3];

        public MonitorRgbLevel(MonitorDevice monitor, LevelParser parser, VcpGetter getter, VcpSetter setter)
        {
            for (uint i = 0; i < 3; i++)
                _values[i] = new MonitorLevel(monitor, parser, getter, setter, i);

            H.Initialize(this);

        }
        public MonitorLevel Channel(uint channel) { return _values[channel]; }

        public MonitorLevel Red => Channel(0);
        public MonitorLevel Green => Channel(1);
        public MonitorLevel Blue => Channel(2);
    }
}