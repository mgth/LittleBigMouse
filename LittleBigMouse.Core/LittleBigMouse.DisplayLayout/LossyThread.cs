/*
  LittleBigMouse.Screen.Config
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Screen.Config.

    LittleBigMouse.Screen.Config is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Screen.Config is distributed in the hope that it will be useful,
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

namespace LittleBigMouse.DisplayLayout;

public class LossyThread
{
    readonly Queue<ThreadStart> _delegates = new Queue<ThreadStart>();

    Thread _thread;
    readonly ThreadStart _finnaly;

    readonly object _threadLock = new object();

    public LossyThread(ThreadStart ts = null)
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
            d = _delegates.Count == 0 ? _finnaly : null;
        }
        d?.DynamicInvoke();
    }
}