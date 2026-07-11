#nullable enable
using System.Runtime.CompilerServices;

namespace HLab.Sys.Windows.MonitorVcp;

public static class ProbeLutExpendScreen
{
    static readonly ConditionalWeakTable<VcpControl, ProbeLut> AllLut = new();

    public static ProbeLut ProbeLut(this VcpControl vcp)
        => AllLut.GetValue(vcp, v => new ProbeLut(v));
}
