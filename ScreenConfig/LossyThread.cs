using System;
using System.Collections.Generic;
using System.Threading;

namespace LbmScreenConfig
{
    public class LossyThread
    {
        private readonly Queue<ThreadStart> _delegates = new Queue<ThreadStart>();

        private Thread _thread;
        private readonly ThreadStart _finnaly;

        private readonly object _threadLock = new object();

        public LossyThread(ThreadStart ts=null)
        {
            _finnaly = ts;
        }

        public void Add(ThreadStart ts)
        {
            lock (_delegates) _delegates.Enqueue(ts);
            new Thread(Run).Start();
        }

        public void Run()
        {
            Thread oldThread = null;

            lock (_threadLock)
            {
                oldThread = _thread;
                _thread = Thread.CurrentThread;
            }

            oldThread?.Join();

            Delegate d = null;
            lock (_delegates)
            {
                while (_delegates.Count > 0)
                    d = _delegates.Dequeue();
            }

            d?.DynamicInvoke();

            lock (_delegates)
            {
                if (_delegates.Count == 0)
                    d = _finnaly;
                else d = null;
            }
            d?.DynamicInvoke();
        }
    }
}

