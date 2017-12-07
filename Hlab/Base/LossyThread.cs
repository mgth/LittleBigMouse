using System;
using System.Collections.Generic;
using System.Threading;

namespace Hlab.Base
{
    public class LossyThread
    {
        private readonly Queue<ThreadStart> _delegates = new Queue<ThreadStart>();

        private Thread _thread;

        private readonly object _threadLock = new object();

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
        }
    }

}

