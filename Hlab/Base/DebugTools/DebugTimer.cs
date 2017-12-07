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
