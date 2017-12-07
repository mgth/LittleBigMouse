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
