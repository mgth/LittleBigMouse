#define TIMER

using System.Collections.Generic;
using System.Linq;

namespace Hlab.Base.DebugTools
{
    public class DebugNotifierData
    {
        public List<long> Ticks = new List<long>();
        public double Frequency = 0;

        public void AddTicks(long ticks)
        {
            
        }

        public double AvgMillis => SumMillis/Ticks.Count;
        public double SumMillis => 1000*Ticks.Sum()/Frequency;
        public double MaxMillis => 1000 * Ticks.Max() / Frequency;
        public double MinMillis => 1000 * Ticks.Min() / Frequency;

        public double MedMillis => 1000*Ticks[Ticks.Count/2]/Frequency;
    }
}
