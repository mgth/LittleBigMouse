using System;
using System.Collections.Generic;

namespace Hlab.Base
{
    public class SuspenderToken : IDisposable
    {
        private readonly Suspender _suspender;
#if DEBUG_SUSPENDER
        public readonly StackTrace StackTrace;
#endif

        public SuspenderToken(Suspender suspender)
        {
            _suspender = suspender;
#if DEBUG_SUSPENDER
            StackTrace = new StackTrace(true);
#endif
        }
        public void Dispose()
        {
            _suspender.Resume(this);
        }
    }
    public class Suspender
    {
        private readonly object _lock = new object();
        private readonly HashSet<SuspenderToken> _list = new HashSet<SuspenderToken>();
        private readonly Action _action;

        public Suspender(Action action=null)
        {
            _action = action;
        }
        public SuspenderToken Get()
        {
            lock (_lock)
            {
                SuspenderToken s = new SuspenderToken(this);
                _list.Add(s);
                return s;
            }
        }

        public bool Suspended
        {
            get
            {
                lock(_lock)
                return _list.Count > 0;
            }            
        }

        public void Resume(SuspenderToken s)
        {
            lock (_lock)
            {
                _list.Remove(s);
                if (_list.Count > 0)
                    return;
            }

//            try
            {
                _action?.Invoke();
            }
//            catch (Exception)
            {
                
            }
        }

    }
}
