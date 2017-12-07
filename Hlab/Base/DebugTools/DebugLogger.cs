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
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Windows;

namespace Hlab.Base.DebugTools
{
    public class DebugLogger : Singleton<DebugLogger>
    {
        public readonly Dictionary<string, DebugNotifierData> Timings = new Dictionary<string, DebugNotifierData>();

        public DebugLogger()
        {
#if TIMER
            Application.Current.Exit += Current_Exit;
#endif
        }

        private void Current_Exit(object sender, ExitEventArgs e)
        {
            Print();
        }

        public void Log(string s, Stopwatch w)
        {

            var f = Stopwatch.Frequency;
            if (!Timings.ContainsKey(s)) Timings[s] = new DebugNotifierData
            {
                Frequency = Stopwatch.Frequency
            };

            Timings[s].Ticks.Add(w.ElapsedTicks);
        }

        public void Print()
        {
            List<KeyValuePair<string, DebugNotifierData>> list = Timings.ToList();

            list.Sort( (kp1,kp2) => kp1.Value.SumMillis.CompareTo(kp2.Value.SumMillis) );

            foreach (var kp in list)
            {

                Write($"{kp.Key}", ConsoleColor.DarkBlue);
                Write($"=>{kp.Value.SumMillis:N2}/{kp.Value.Ticks.Count} = {kp.Value.MinMillis:N2}<{kp.Value.AvgMillis:N2}<{kp.Value.MaxMillis:N2}({kp.Value.MedMillis:N2})\n");
            }
        }

        private void Write(string s, ConsoleColor? c=null)
        {
            if(!c.HasValue) c = ConsoleColor.Black;
            var old = Console.ForegroundColor;

            Console.ForegroundColor = c.Value;

            Debug.Print(s);
            Console.Write(s);

            Console.ForegroundColor = old;
        }
    }
}
