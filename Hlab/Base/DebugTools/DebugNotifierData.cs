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
