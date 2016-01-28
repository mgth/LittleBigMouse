using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using LbmScreenConfig;

namespace LittleBigMouse_Control.Argyll
{
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

        internal class Argyll
        {
            private IniFile _dispcalIni;
            private IniFile DispcalIni
            {
                get
                {
                    if (_dispcalIni==null)
                    _dispcalIni = new IniFile(
                        Path.Combine(
                            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                            @"dispcalGui\dispcalGUI.ini"
                            ));

                    return _dispcalIni;
                }

            }

            private string ArgyllPath => DispcalIni.ReadValue("Default", "argyll.dir","");

            private string Observer => DispcalIni.ReadValue("Default", "observer", "1931_2");
            private int ColorTemp => int.Parse(DispcalIni.ReadValue("Default", "whitepoint.colortemp","5000"));

            private readonly double[] _xyz = { 0, 0, 0 };
//            private readonly double[] _lab = { 0, 0, 0 };

            private static void ArgyllSendKey(Process p, String key)
            {
                //System.Threading.Thread.Sleep(300);
                p.StandardInput.Flush();
                p.StandardInput.Write(key);
                p.StandardInput.Flush();
            }

            private bool _calibrating = false;

            private void ArgyllOutputHandler(object sendingProcess, DataReceivedEventArgs outLine)
            {
                System.Threading.Thread.CurrentThread.CurrentCulture = new System.Globalization.CultureInfo("en-US");

                string line = outLine.Data;

                Console.WriteLine(line);

                if (line == null) return;

                Process p = sendingProcess as Process;

                if (p == null) return;

                if (line.Contains("Error - Opening USB port"))
                    ArgyllSendKey(p, "q");

                if (line.Contains("calibration position"))
                {
                    if (!_calibrating)
                    {
                        var result = MessageBox.Show("Place instrument in calibration position", "Instrument",
                            MessageBoxButton.OKCancel, MessageBoxImage.Information);
                        ArgyllSendKey(p, result == MessageBoxResult.OK ? "k" : "q");

                        _calibrating = true;
                    }
                    else ArgyllSendKey(p, "k");
                }

                if (line.Contains("Place instrument"))
                {
                    System.Threading.Thread.Sleep(300);
                    p.StandardInput.Flush();
                    //var result = MessageBox.Show("Place instrument in measure position", "Instrument",
                    //    MessageBoxButton.OKCancel, MessageBoxImage.Information);
                    //ArgyllSendKey(p, result == MessageBoxResult.OK ? "0" : "q");
                    ArgyllSendKey(p, "0");
                }

                if (line.Contains("Result is XYZ:"))
                {
                    int pos = line.IndexOf("XYZ: ", StringComparison.Ordinal);
                    string sub = line.Substring(pos + 5);
                    sub = sub.Remove(sub.IndexOf(','));
                    string[] s = sub.Split(' ');
                    for (int i = 0; i < 3; i++)
                    {
                        try
                        {
                            _xyz[i] = Double.Parse(s[i]);
                        }
                        catch { _xyz[i] = 0; }
                    }

                    _calibrating = false;
                    //if (line.Contains("D50 Lab:"))
                    //{
                    //    pos = line.IndexOf("D50 Lab:", StringComparison.Ordinal);
                    //    sub = line.Substring(pos + 9);
                    //    //sub.Remove(sub.IndexOf(','));
                    //    s = sub.Split(' ');
                    //    for (int i = 0; i < 3; i++)
                    //    {
                    //        try
                    //        {
                    //            _lab[i] = Double.Parse(s[i]);
                    //        }
                    //        catch { _lab[i] = 0; }
                    //    }

                    //}

                    //((Process)sendingProcess).Kill();
                }
            }

            public string SpotReadArgs
            {
                get
                {
                    string s = " -N";
                    switch (DispcalIni.ReadValue("Default", "measurement_mode", "1"))
                    {
                        case "c": // CRT ???
                            break;
                        case "p": // CRT ???
                            s += " -p";
                            break;
                        case "1":
                            s += " -e";
                            break;
                    }
                        


                    if (DispcalIni.ReadValue("Default", "measurement_mode.highres","0") == "1")
                        s += " -H";

                    if (DispcalIni.ReadValue("Default", "measurement_mode.adaptive","1") == "0")
                        s += " -Y A";
                    
                    s += " -O";

                    s += " -Q " + Observer;

                    return s;
                }
        }

            public bool Installed => ArgyllPath != "";

            public ProbedColorXYZ SpotRead()
            {
                if (!Installed) return null;

                do
                {
                    ExecSpotRead();
                } while (_calibrating);


                return new ProbedColorXYZ
                {
                    Illuminant = ProbedColor.DIlluminant(ColorTemp),
                    X = _xyz[0],
                    Y = _xyz[1],
                    Z = _xyz[2]
                };
            }

            public void ExecSpotRead()
            {

                Process[] aProc = Process.GetProcessesByName("Spotread");
                for (int i = 0; i < aProc.Length; i++)
                {
                    aProc[i].Kill();
                    if (!aProc[i].HasExited)
                        aProc[i].WaitForExit();
                }

                Process p = new Process();

                p.StartInfo.FileName = Path.Combine(ArgyllPath , @"Spotread.exe");
                //                p.StartInfo.Arguments = "-N -O -Y A";
                p.StartInfo.Arguments = SpotReadArgs;
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

                //Console.WriteLine("XYZ = " + _xyz[0].ToString() + ' ' + _xyz[1].ToString() + ' ' + _xyz[2].ToString());
                //Console.WriteLine("x = " + _xyz[0] / (_xyz[0] + _xyz[1] + _xyz[2]) + "  y = " + _xyz[1] / (_xyz[0] + _xyz[1] + _xyz[2]));
                //Console.WriteLine(d50[0].ToString() + ' ' + d50[1].ToString() + ' ' + d50[2].ToString());

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
       p.StartInfo.FileName = "C:\\Fabien\\Argyll_V1.4.0\\bin\\Spotread.exe";
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
}
