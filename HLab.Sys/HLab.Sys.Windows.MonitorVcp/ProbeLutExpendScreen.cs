using System.Collections.Generic;
using HLab.Sys.Windows.Monitors;

namespace HLab.Sys.Windows.MonitorVcp
{
    public static class ProbeLutExpendScreen
    {
        private static readonly Dictionary<MonitorDevice, ProbeLut> AllLut = new Dictionary<MonitorDevice, ProbeLut>();
        public static ProbeLut ProbeLut(this MonitorDevice monitor)
        {
            if (!AllLut.ContainsKey(monitor))
            {
                AllLut.Add(monitor, new ProbeLut(monitor));                   
            }
            return AllLut[monitor];
        }
    }
}