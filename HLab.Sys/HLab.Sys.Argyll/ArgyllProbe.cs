/*
  HLab.Argyll
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Argyll.

    HLab.Argyll is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Argyll is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using ReactiveUI;
using System;
using System.Collections.ObjectModel;
using System.Diagnostics;
using System.IO;
using System.Xml.Serialization;

namespace HLab.Sys.Argyll;

public class ArgyllProbe : ReactiveObject
{
    public ArgyllProbe()
    {
        ConfigFromDipcalGUI();
    }
    public ArgyllProbe(bool autoconfig = true)
    {
        if(autoconfig) ConfigFromDipcalGUI();
    }


    private static IniFile _dispcalIni;
    private static IniFile DispcalIni
    {
        get
        {
            if (_dispcalIni == null)
                _dispcalIni = new IniFile(
                    Path.Combine(
                        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                        @"DisplayCAL\DisplayCAL.ini"
                    ));

            return _dispcalIni;
        }

    }


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
    private bool _spectrum = false;

    public string Name
    {
        get => _name; 
        set => this.RaiseAndSetIfChanged(ref _name, value);
    }
    string _name;

    public double SpectrumFrom
    {
        get => _spectrumFrom; 
        set => this.RaiseAndSetIfChanged(ref _spectrumFrom, value);
    }
    double _spectrumFrom;

    public double SpectrumTo
    {
        get => _spectrumTo; 
        set => this.RaiseAndSetIfChanged(ref _spectrumTo, value);
    }
    double _spectrumTo;//c => c.Default(720.0));

    public int SpectrumSteps
    {
        get => _spectrumSteps; 
        set => this.RaiseAndSetIfChanged(ref _spectrumSteps, value);
    }
    int _spectrumSteps;

    public double Cct
    {
        get => _cct; 
        set => this.RaiseAndSetIfChanged(ref _cct, value);
    }
    double _cct;

    public double Cri
    {
        get => _cri; 
        set => this.RaiseAndSetIfChanged(ref _cri, value);
    }
    double _cri;

    public double Tlci
    {
        get => _tlci; 
        set => this.RaiseAndSetIfChanged(ref _tlci, value);
    }
    double _tlci;

    public double Lux
    {
        get => _lux; 
        set => this.RaiseAndSetIfChanged(ref _lux, value);
    }
    double _lux;

    public ObservableCollection<double> Spectrum { get; set; } = new ObservableCollection<double> {0};

    public ObservableCollection<double> WaveLength { get; set; } = new ObservableCollection<double> {0};
    private void ArgyllOutputHandler(object sendingProcess, DataReceivedEventArgs outLine)
    {
        System.Threading.Thread.CurrentThread.CurrentCulture = new System.Globalization.CultureInfo("en-US");

        var line = outLine.Data;

        Console.WriteLine(line);

        if (line == null) return;

        if (sendingProcess is not Process p) return;

        if (_spectrum)
        {
            var s = line.Split(',');

            Spectrum.Clear();
            WaveLength.Clear();

            var nm = SpectrumFrom;
            var step = (SpectrumTo - SpectrumFrom)/(SpectrumSteps - 1);

            foreach (var t in s)
            {
                Spectrum.Add( double.Parse(t));
                WaveLength.Add(nm);
                nm += step;
            }
            _spectrum = false;
        }


        if (line.Contains("Spectrum from"))
        {
            var pos = line.IndexOf("Spectrum from", StringComparison.Ordinal);
            var sub = line[(pos + 14)..];
            var s = sub.Split(' ');
            SpectrumFrom = double.Parse(s[0]);
            SpectrumTo = double.Parse(s[2]);
            SpectrumSteps = int.Parse(s[5]);
            _spectrum = true;
        }

        if (line.Contains("Ambient"))
        {
            string[] s = line.Split(' ');
            Lux = double.Parse(s[3]);
            Cct = double.Parse(s[7].Replace("K",""));
        }

        if (line.Contains("(Ra)"))
        {
            string[] s = line.Split(' ');
            Cri = double.Parse(s[6]);
        }

        if (line.Contains("(Qa)"))
        {
            string[] s = line.Split(' ');
            Tlci = double.Parse(s[8]);
        }

        if (line.Contains("Error - Opening USB port"))
            ArgyllSendKey(p, "q");

        if (line.Contains("calibration position"))
        {
            if (!_calibrating)
            {
                // TODO
                //var result = MessageBox.Show("Place instrument in calibration position", "Instrument",
                //    MessageBoxButton.OKCancel, MessageBoxImage.Information);
                //ArgyllSendKey(p, result == MessageBoxResult.OK ? "k" : "q");

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

    public enum MeasurementMode
    {
        Emissive,
        Projector,
        Ambiant,
        Flash
    }

    public enum ObserverEnum
    {
        CIE_1931_2,
        CIE_1964_10,
        CIE_2012_10,
        CIE_2012_2,
        SB_1955_2,
        JV_1978_2,
        Shaw,
    }

    public static string ArgyllPath { get; set; }

    public int ColorTemp { get; set; } = 6500;
    public MeasurementMode Mode { get; set; } = MeasurementMode.Emissive;
    public bool HighResolution { get; set; } = true;
    public bool Adaptive { get; set; } = true;
    public bool ReadSpectrum { get; set; } = false;
    public bool ReadCri { get; set; } = false;

    private ObserverEnum Observer { get; set; } = ObserverEnum.CIE_1931_2;

    public static void PathFromDispcalGUI()
    {
        ArgyllPath = DispcalIni.ReadValue("Default", "argyll.dir", "");
    }

    public void ConfigFromDipcalGUI()
    {
        PathFromDispcalGUI();

        ColorTemp = 
            int.Parse(DispcalIni.ReadValue("Default", "whitepoint.colortemp", "5000"));

        switch (DispcalIni.ReadValue("Default", "measurement_mode", "1"))
        {
            case "c": // CRT ???
                break;
            case "p": // CRT ???
                Mode = MeasurementMode.Projector;
                break;
            case "1":
                Mode = MeasurementMode.Emissive;
                break;
        }

        HighResolution = (DispcalIni.ReadValue("Default", "measurement_mode.highres", "0") == "1");

        Adaptive = (DispcalIni.ReadValue("Default", "measurement_mode.adaptive", "1") == "1");

        string obs = DispcalIni.ReadValue("Default", "observer", "1931_2");
        switch (obs)
        {
            case "1931_2":
                Observer = ObserverEnum.CIE_1931_2;
                break;
            case "1964_10":
                Observer = ObserverEnum.CIE_1964_10;
                break;
            case "1955_2":
                Observer = ObserverEnum.SB_1955_2;
                break;
            case "shaw":
                Observer = ObserverEnum.Shaw;
                break;
            case "1978_2":
                Observer = ObserverEnum.JV_1978_2;
                break;
            case "2012_2":
                Observer = ObserverEnum.CIE_2012_2;
                break;
            case "2012_10":
                Observer = ObserverEnum.CIE_2012_10;
                break;
        }
    }

    public string SpotReadArgs
    {
        get
        {
            string s = " -N";
            switch (Mode)
            {
                case MeasurementMode.Projector:
                    s += " -pb";
                    break;
                case MeasurementMode.Emissive:
                    s += " -e";
                    break;
                case MeasurementMode.Ambiant:
                    s += " -a";
                    break;
                case MeasurementMode.Flash:
                    s += " -f";
                    break;
            }



            if (HighResolution) s += " -H";

            if (!Adaptive) s += " -Y A";

            s += " -O";

            s += " -Q";

            switch (Observer)
            {
                case ObserverEnum.CIE_1931_2:
                    s += " 1931_2";
                    break;
                case ObserverEnum.CIE_1964_10:
                    s += " 1964_10";
                    break;
                case ObserverEnum.SB_1955_2:
                    s += " 1955_2";
                    break;
                case ObserverEnum.Shaw:
                    s += " shaw";
                    break;
                case ObserverEnum.JV_1978_2:
                    s += " 1978_2";
                    break;
                default:
                    throw new ArgumentOutOfRangeException();
            }

            if (ReadSpectrum) s += " -s";
            if (ReadCri) s += " -T";
            return s;
        }
    }


    public bool Installed => ArgyllPath != "";

    public bool SpotRead()
    {
        if (!Installed) return false;

        do
        {
            ExecSpotRead();
        } while (_calibrating);

        return true;
    }

    public ProbedColor ProbedColor => new ProbedColorXYZ
    {
        Illuminant = ProbedColor.DIlluminant(ColorTemp),
        X = _xyz[0],
        Y = _xyz[1],
        Z = _xyz[2]
    };

    public void ExecSpotRead()
    {
        var aProc = Process.GetProcessesByName("Spotread");
        foreach (var t in aProc)
        {
            try
            {
                t.Kill();
                if (!t.HasExited)
                    t.WaitForExit();
                    
            }
            catch (Exception) { }
        }

        var p = new Process
        {
            StartInfo =
            {
                FileName = Path.Combine(ArgyllPath, @"Spotread.exe"),
                Arguments = SpotReadArgs,
                UseShellExecute = false,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                RedirectStandardInput = true,
                CreateNoWindow = true
            }
        };

        //                p.StartInfo.Arguments = "-N -O -Y A";

        try
        {
            p.StartInfo.EnvironmentVariables.Add("ARGYLL_NOT_INTERACTIVE", "yes");
        }
        catch
        {
        }

        p.ErrorDataReceived += ArgyllOutputHandler;
        p.OutputDataReceived += ArgyllOutputHandler;

        p.Start();
        p.BeginErrorReadLine();
        p.BeginOutputReadLine();

        if (!p.HasExited) p.WaitForExit();
    }

    //TODO

    public void Save()
    {
        //    Microsoft.Win32.SaveFileDialog dlg = new Microsoft.Win32.SaveFileDialog();
        //    dlg.DefaultExt = ".probe";
        //    dlg.Filter = "Probe documents (.probe)|*.probe";
        //    bool? result = dlg.ShowDialog();
        //    if (result == true)
        //    {
        //        // Open document
        //        string filename = dlg.FileName;
        //        Save(filename);
        //    }
    }

    public void Save(string path)
    {
        XmlSerializer serializer = new XmlSerializer(typeof(ArgyllProbe));
        using (TextWriter writer = new StreamWriter(path))
        {
            serializer.Serialize(writer, this);
        }
    }


    public static ArgyllProbe Load()
    {
        //TODO
        //Microsoft.Win32.OpenFileDialog dlg = new Microsoft.Win32.OpenFileDialog();
        //dlg.DefaultExt = ".probe";
        //dlg.Filter = "Probe documents (.probe)|*.probe";
        //bool? result = dlg.ShowDialog();
        //if (result == true)
        //{
        //    // Open document
        //    string filename = dlg.FileName;
        //    return Load(filename);
        //}
        return null;
    }


    public static ArgyllProbe Load(string path)
    {
        ArgyllProbe probe = null;

        XmlSerializer deserializer = new XmlSerializer(typeof(ArgyllProbe));

        try
        {
            TextReader reader = new StreamReader(path);
            probe = (ArgyllProbe)deserializer.Deserialize(reader);
            reader.Close();
        }
        catch (FileNotFoundException)
        {

        }

        return probe;
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

