using System.Collections.Generic;
using HLab.Sys.Windows.Monitors;

namespace HLab.Sys.Windows.MonitorVcp;

public static class ProbeLutExpendScreen
{
    static readonly Dictionary<MonitorDevice, ProbeLut> AllLut = new();
    public static ProbeLut ProbeLut(this MonitorDevice monitor)
    {
        if (!AllLut.ContainsKey(monitor))
        {
            AllLut.Add(monitor, new ProbeLut(monitor));                   
        }
        return AllLut[monitor];
    }
}