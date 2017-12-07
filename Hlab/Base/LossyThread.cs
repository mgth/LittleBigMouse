/*
  Hlab.Base
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of Hlab.Base.

    Hlab.Base is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    Hlab.Base is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
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

