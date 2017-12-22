/*
  HLab.Base
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Base.

    HLab.Base is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Base is distributed in the hope that it will be useful,
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

namespace HLab.Base
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
