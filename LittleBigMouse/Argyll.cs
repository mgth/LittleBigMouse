using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace LittleBigMouse
{
    using System;
    using System.Collections.Generic;
    using System.Linq;
    using System.Text;
    using System.Diagnostics;
    using System.Windows;

        class Argyll
        {
            private string _ArgyllPath = @"F:\bin\Argyll\";
            private double[] _XYZ = { 0, 0, 0 };
            private double[] _Lab = { 0, 0, 0 };

            private void ArgyllSendKey(Process p, String key)
            {
                //System.Threading.Thread.Sleep(300);
                p.StandardInput.Flush();
                p.StandardInput.Write(key);
                p.StandardInput.Flush();
            }

            private void ArgyllOutputHandler(object sendingProcess, DataReceivedEventArgs outLine)
            {
                System.Threading.Thread.CurrentThread.CurrentCulture = new System.Globalization.CultureInfo("en-US");

                string line = outLine.Data;

                Console.WriteLine(line);

                if (line == null) return;

                if (line.Contains("Error - Opening USB port"))
                    ArgyllSendKey((Process)sendingProcess, "q");

                if (line.Contains("calibration position"))
                    ArgyllSendKey((Process)sendingProcess, "k");

                if (line.Contains("Place instrument"))
                {
                    System.Threading.Thread.Sleep(300);
                    ((Process)sendingProcess).StandardInput.Flush();
                    ArgyllSendKey((Process)sendingProcess, "0");
                }

                if (line.Contains("Result is XYZ:"))
                {
                    int pos = line.IndexOf("XYZ: ");
                    string sub = line.Substring(pos + 5);
                    sub = sub.Remove(sub.IndexOf(','));
                    string[] s = sub.Split(' ');
                    for (int i = 0; i < 3; i++)
                    {
                        try
                        {
                            _XYZ[i] = Double.Parse(s[i]);
                        }
                        catch { _XYZ[i] = 0; }
                    }

                    if (line.Contains("D50 Lab:"))
                    {
                        pos = line.IndexOf("D50 Lab:");
                        sub = line.Substring(pos + 9);
                        //sub.Remove(sub.IndexOf(','));
                        s = sub.Split(' ');
                        for (int i = 0; i < 3; i++)
                        {
                            try
                            {
                                _Lab[i] = Double.Parse(s[i]);
                            }
                            catch { _Lab[i] = 0; }
                        }

                    }

                   //((Process)sendingProcess).Kill();
                }
            }


            public double[] spotread()
            {
                Process[] aProc = Process.GetProcessesByName("spotread");
                for (int i = 0; i < aProc.Length; i++)
                {
                    aProc[i].Kill();
                    if (!aProc[i].HasExited)
                        aProc[i].WaitForExit();
                }

                Process p = new Process();

                p.StartInfo.FileName = _ArgyllPath + @"bin\spotread.exe";
//                p.StartInfo.Arguments = "-N -O -Y A";
                p.StartInfo.Arguments = "-e -N -H -O";
                p.StartInfo.UseShellExecute = false;
                p.StartInfo.RedirectStandardOutput = true;
                p.StartInfo.RedirectStandardError = true;
                p.StartInfo.RedirectStandardInput = true;
                p.StartInfo.CreateNoWindow = true;

                try
                {
                    p.StartInfo.EnvironmentVariables.Add("ARGYLL_NOT_INTERACTIVE", "yes");
                }
                catch { }

                p.ErrorDataReceived += new DataReceivedEventHandler(ArgyllOutputHandler);
                p.OutputDataReceived += new DataReceivedEventHandler(ArgyllOutputHandler);

                p.Start();
                p.BeginErrorReadLine();
                p.BeginOutputReadLine();

                if (!p.HasExited) p.WaitForExit();

                Console.WriteLine("XYZ = " + _XYZ[0].ToString() + ' ' + _XYZ[1].ToString() + ' ' + _XYZ[2].ToString());
                Console.WriteLine("x = " + _XYZ[0] / (_XYZ[0] + _XYZ[1] + _XYZ[2]) + "  y = " + _XYZ[1] / (_XYZ[0] + _XYZ[1] + _XYZ[2]));
            //Console.WriteLine(d50[0].ToString() + ' ' + d50[1].ToString() + ' ' + d50[2].ToString());

            return _XYZ;
            }
        }
    }

    /*



    private int state = 0;
    private double[] xyz = { 0, 0, 0 };
    private double[] rvb = { 0, 0, 0 };
    private double[] d50 = { 0, 0, 0 };

    private void ArgyllOutputHandler(object sendingProcess, DataReceivedEventArgs outLine)
    {
       System.Threading.Thread.CurrentThread.CurrentCulture = new System.Globalization.CultureInfo("en-US");

       string line = outLine.Data;

       Console.WriteLine(line);

       if(line == null) return;
       if(line.Contains("Error - Opening USB port")) state = 255;
       if(line.Contains("needs a calibration")) state = -1;
       if(line.Contains("Place instrument")) state = 1;
       if(line.Contains("Result is XYZ:"))
       {
          int pos = line.IndexOf("XYZ: ");
          string sub = line.Substring(pos + 5);
          sub = sub.Remove(sub.IndexOf(','));
          string[] s = sub.Split(' ');
          for(int i = 0; i < 3; i++)
          {
             try
             {
                xyz[i] = Double.Parse(s[i]);
             }
             catch { xyz[i] = 0; }
          }

          if(line.Contains("D50 Lab:"))
          {
             pos = line.IndexOf("D50 Lab:");
             sub = line.Substring(pos + 9);
             //sub.Remove(sub.IndexOf(','));
             s = sub.Split(' ');
             for(int i = 0; i < 3; i++)
             {
                try
                {
                   d50[i] = Double.Parse(s[i]);
                }
                catch { d50[i] = 0; }
             }

          }
          state = 2;
       }
    }

    public void Lance()
    {
       Process p = new Process();

       p.StartInfo.FileName = "C:\\Fabien\\Argyll_V1.4.0\\bin\\spotread.exe";
       p.StartInfo.Arguments = "-N -d";
       //p.StartInfo.Arguments = "-N -H -d";
       //p.StartInfo.Arguments = "--help";
       p.StartInfo.UseShellExecute = false;
       p.StartInfo.RedirectStandardOutput = true;
       p.StartInfo.RedirectStandardError = true;
       p.StartInfo.RedirectStandardInput = true;
       p.StartInfo.CreateNoWindow = true;

       p.StartInfo.EnvironmentVariables.Add("ARGYLL_NOT_INTERACTIVE", "yes");

       p.ErrorDataReceived += new DataReceivedEventHandler(ArgyllOutputHandler);
       p.OutputDataReceived += new DataReceivedEventHandler(ArgyllOutputHandler);

       state = 0;

       p.Start();
       p.BeginErrorReadLine();
       p.BeginOutputReadLine();

       while(state == 0) { }

       if(state == -1)
       {
          p.StandardInput.Write("k");
          p.StandardInput.Flush();
          Console.Write(state);
       }
       while(state == -1) { }

       if(state > 100) return;

       if(state == 1)
       {
          p.StandardInput.Write("0");
          p.StandardInput.Flush();
       }
       while(state == 1) { }
       try
       {
          Console.WriteLine(xyz[0].ToString() + ' ' + xyz[1].ToString() + ' ' + xyz[2].ToString());

          //rvb[0] = 





          Console.WriteLine(rvb[0].ToString() + ' ' + rvb[1].ToString() + ' ' + rvb[2].ToString());
          Console.WriteLine(d50[0].ToString() + ' ' + d50[1].ToString() + ' ' + d50[2].ToString());
       }
       catch
       {
       }
       p.StandardInput.Write("q");
       p.StandardInput.Flush();

       if(!p.HasExited) p.Kill();

       if(!p.HasExited) p.WaitForExit();
    }

 }
}
*/

