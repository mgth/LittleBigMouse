#define TIMER

using System;
using System.Diagnostics;

namespace Hlab.Base.DebugTools
{
    public class DebugTimer : IDisposable
    {
#if TIMER
        private readonly Stopwatch _sw;
        private readonly string _name;
#endif

        public DebugTimer(string name)
        {
#if TIMER
           _name = name;
            _sw = new Stopwatch();
            _sw.Start();
#endif
        }

        public void Dispose()
        {
#if TIMER
            _sw.Stop();
            DebugLogger.D.Log(_name,_sw);
#endif
        }
    }
}
