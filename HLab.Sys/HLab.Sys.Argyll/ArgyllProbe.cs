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
using System.Threading;
using System.Xml.Serialization;

namespace HLab.Sys.Argyll;

public class ArgyllProbe : ReactiveObject
{
   public ArgyllProbe()
   {
      TryConfigFromDispcalGUI();
   }
   public ArgyllProbe(bool autoconfig = true)
   {
      if (autoconfig) TryConfigFromDispcalGUI();
   }

   // A broken or exotic DisplayCAL config must not take down whoever
   // instantiates the probe (the VCP view-model creates one per monitor panel):
   // fall back to defaults.
   void TryConfigFromDispcalGUI()
   {
      try
      {
         ConfigFromDipcalGUI();
      }
      catch (Exception)
      {
      }
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
      // spotread may die between emitting a line and this reply (wrong sensor
      // position, USB hiccup): a write to the dead pipe must not take the app
      // down with it — this runs on a thread-pool thread, past any UI handler.
      try
      {
         p.StandardInput.Flush();
         p.StandardInput.Write(key);
         p.StandardInput.Flush();
      }
      catch (Exception)
      {
      }
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

   public string Message
   {
      get => _message;
      set => this.RaiseAndSetIfChanged(ref _message, value);
   }
   string _message;

   public ObservableCollection<double> Spectrum { get; set; } = new ObservableCollection<double> { 0 };

   public ObservableCollection<double> WaveLength { get; set; } = new ObservableCollection<double> { 0 };
   private void ArgyllOutputHandler(object sendingProcess, DataReceivedEventArgs outLine)
   {
      // Thread-pool callback: any escaping exception is fatal for the whole
      // app, so parse defensively and log instead of throwing.
      try
      {
         ArgyllOutputHandlerCore(sendingProcess, outLine);
      }
      catch (Exception ex)
      {
         Console.Error.WriteLine($"ArgyllProbe: failed to handle spotread output: {ex.Message}");
      }
   }

   void ArgyllOutputHandlerCore(object sendingProcess, DataReceivedEventArgs outLine)
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
         var step = (SpectrumTo - SpectrumFrom) / (SpectrumSteps - 1);

         foreach (var t in s)
         {
            Spectrum.Add(double.Parse(t));
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
         Cct = double.Parse(s[7].Replace("K", ""));
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
            Message = "Place instrument in calibration position";
            _calibrating = true;
         }

         // Acknowledge every prompt, first included: the instrument's position
         // sensor gates the calibration anyway (wrong position → the prompt
         // comes back), while an unanswered prompt can wait forever.
         // NEVER send 'k' here: any key answers a prompt, but a stray 'k'
         // landing on the main menu starts a whole new calibration.
         Thread.Sleep(300);
         ArgyllSendKey(p, " ");
      }

      if (line.Contains("Calibration complete") || line.Contains("Calibration failed"))
      {
         // Leaving _calibrating set would keep the keepalive keys flowing;
         // queued during the calibration measurement (spotread reads no input
         // while measuring), they would each trigger another calibration once
         // back at the menu.
         _calibrating = false;
         Message = line.Contains("failed") ? "Calibration failed" : "Calibration done";
      }

      // Readings refused until the dial reaches the right notch: spotread says
      // "Spot read failed due to the sensor being in the wrong position
      //  (Sensor should be in surface position)" and the auto-answered menu
      // retries — without this message the loop looks like a silent hang.
      if (line.Contains("wrong position") || line.Contains("Sensor should be") || line.Contains("measurement position"))
      {
         Message = _calibrating
            ? "Sensor in wrong position — set it to the calibration position"
            : "Sensor in wrong position — set it to the measure (surface) position";
      }

      // Main menu displayed: the session is ready for a command. Readings are
      // triggered explicitly by SpotRead, never auto-answered here.
      // The actual "Hit ESC or Q…" prompt CANNOT be the trigger: it has no
      // trailing newline, so it only reaches this line-based handler once the
      // next output completes it — after a reading. Key on the last
      // newline-terminated line of the menu instead.
      if (line.Contains("'k' to do a calibration"))
      {
         Interlocked.Increment(ref _menuSeq);
         _ready.Set();
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
         _result.Set();
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

   static string ExeName => OperatingSystem.IsWindows() ? "spotread.exe" : "spotread";

   /// <summary>
   /// Full path of the spotread executable, or null when Argyll is not
   /// available. DisplayCAL's argyll.dir wins when it holds a usable install
   /// (the key points either at the bin directory or at the Argyll root);
   /// otherwise the system PATH is searched — on Linux Argyll is typically a
   /// distro package with no DisplayCAL entry at all.
   /// </summary>
   public static string SpotreadPath
   {
      get
      {
         if (!string.IsNullOrEmpty(ArgyllPath))
         {
            var candidate = Path.Combine(ArgyllPath, ExeName);
            if (File.Exists(candidate)) return candidate;

            candidate = Path.Combine(ArgyllPath, "bin", ExeName);
            if (File.Exists(candidate)) return candidate;
         }

         var paths = Environment.GetEnvironmentVariable("PATH");
         if (paths is null) return null;

         foreach (var dir in paths.Split(Path.PathSeparator))
         {
            if (string.IsNullOrWhiteSpace(dir)) continue;
            try
            {
               var candidate = Path.Combine(dir, ExeName);
               if (File.Exists(candidate)) return candidate;
            }
            catch (Exception)
            {
               // unparsable PATH entry
            }
         }

         return null;
      }
   }

   /// <summary>Target white point in kelvin — the D(T) illuminant the tuning aims at, not a spotread argument.</summary>
   public double ColorTemp
   {
      get => _colorTemp;
      set => this.RaiseAndSetIfChanged(ref _colorTemp, value);
   }
   double _colorTemp = 6500;

   public MeasurementMode Mode { get; set; } = MeasurementMode.Emissive;
   public bool HighResolution { get; set; } = true;

   /// <summary>Command-line parameter: changing it restarts the session on the next reading.</summary>
   public bool Adaptive
   {
      get => _adaptive;
      set
      {
         if (_adaptive == value) return;
         _adaptive = value;
         InvalidateSession();
      }
   }
   bool _adaptive = true;

   public bool ReadSpectrum { get; set; } = false;
   public bool ReadCri { get; set; } = false;

   public ObserverEnum Observer
   {
      get => _observer;
      set
      {
         if (_observer == value) return;
         this.RaiseAndSetIfChanged(ref _observer, value);
         this.RaisePropertyChanged(nameof(SpotReadArgs));
         // observer lives on the command line: restart the session lazily
         InvalidateSession();
      }
   }
   ObserverEnum _observer = ObserverEnum.CIE_1931_2;

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

         // no -O (single reading then exit): the process is kept alive as a
         // session, one launch pays the USB enumeration for many readings

         s += " -Q";

         s += Observer switch
         {
            ObserverEnum.CIE_1931_2 => " 1931_2",
            ObserverEnum.CIE_1964_10 => " 1964_10",
            ObserverEnum.SB_1955_2 => " 1955_2",
            ObserverEnum.Shaw => " shaw",
            ObserverEnum.JV_1978_2 => " 1978_2",
            // Argyll 3.5 renamed the cone-fundamental observers to the CIE
            // 170-2:2015 nomenclature; older versions only accept 2012_*
            ObserverEnum.CIE_2012_2 => Uses2015ObserverNames ? " 2015_2" : " 2012_2",
            ObserverEnum.CIE_2012_10 => Uses2015ObserverNames ? " 2015_10" : " 2012_10",
            _ => throw new ArgumentOutOfRangeException()
         };

         if (ReadSpectrum) s += " -s";
         if (ReadCri) s += " -T";
         return s;
      }
   }


   public bool Installed => SpotreadPath is not null;

   static bool? _uses2015ObserverNames;

   /// <summary>
   /// Argyll 3.5 renamed the -Q observers 2012_* to 2015_* and rejects the old
   /// spelling with a usage dump. Probed once from the usage text (spotread -?
   /// answers instantly, unlike a real run which enumerates instruments first).
   /// </summary>
   static bool Uses2015ObserverNames
   {
      get
      {
         if (_uses2015ObserverNames is { } known) return known;

         var result = false;
         try
         {
            var exe = SpotreadPath;
            if (exe is not null)
            {
               using var p = Process.Start(new ProcessStartInfo(exe, "-?")
               {
                  UseShellExecute = false,
                  RedirectStandardOutput = true,
                  RedirectStandardError = true,
                  CreateNoWindow = true,
               });
               if (p is not null)
               {
                  var text = p.StandardOutput.ReadToEnd() + p.StandardError.ReadToEnd();
                  if (!p.WaitForExit(5000)) p.Kill(entireProcessTree: true);
                  result = text.Contains("2015_2");
               }
            }
         }
         catch (Exception)
         {
         }

         _uses2015ObserverNames = result;
         return result;
      }
   }

   volatile bool _abort;
   volatile Process _current;

   /// <summary>True from Abort() until ResetAbort(): pending reads bail out.</summary>
   public bool Aborted => _abort;

   public void ResetAbort() => _abort = false;

   /// <summary>
   /// Stop the current spotread run (kills the process — also the way out of
   /// the auto-answered wrong-sensor-position retry loop) and make subsequent
   /// SpotRead calls return false until ResetAbort().
   /// </summary>
   public void Abort()
   {
      _abort = true;

      var p = _current;
      try
      {
         if (p is { HasExited: false }) p.Kill(entireProcessTree: true);
      }
      catch (Exception)
      {
      }
   }

   // --- persistent spotread session ------------------------------------------
   // One spotread process serves many readings. Spawning one per reading paid
   // the full instrument enumeration every launch — 20s+ on machines where a
   // USB hub answers descriptor reads slowly — while a reading itself takes
   // well under a second.

   readonly ManualResetEventSlim _ready = new(false);   // main menu prompt seen
   readonly ManualResetEventSlim _result = new(false);  // XYZ line received
   int _menuSeq;

   static readonly bool PerfTrace =
      Environment.GetEnvironmentVariable("LBM_PERF") is "1" or "true" or "yes";

   /// <summary>Kill the running session — used when a command-line parameter
   /// (observer, adaptive) changes; the next reading restarts it.</summary>
   public void InvalidateSession()
   {
      var p = _current;
      try
      {
         if (p is { HasExited: false }) p.Kill(entireProcessTree: true);
      }
      catch (Exception)
      {
      }
   }

   bool EnsureSession()
   {
      var p = _current;
      if (p is { HasExited: false }) return true;

      var exe = SpotreadPath;
      if (exe is null) return false;

      foreach (var name in new[] { "spotread", "Spotread" })
      foreach (var stray in Process.GetProcessesByName(name))
      {
         try
         {
            stray.Kill();
            if (!stray.HasExited) stray.WaitForExit();
         }
         catch (Exception) { }
      }

      _ready.Reset();
      _result.Reset();
      _calibrating = false;

      var sw = PerfTrace ? Stopwatch.StartNew() : null;

      p = new Process
      {
         StartInfo =
            {
                UseShellExecute = false,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                RedirectStandardInput = true,
                CreateNoWindow = true
            }
      };

      if (OperatingSystem.IsWindows())
      {
         p.StartInfo.FileName = exe;
         p.StartInfo.Arguments = SpotReadArgs;
         try
         {
            p.StartInfo.EnvironmentVariables.Add("ARGYLL_NOT_INTERACTIVE", "yes");
         }
         catch
         {
         }
      }
      else
      {
         // Argyll 3.5+ dumps its usage and exits when ARGYLL_NOT_INTERACTIVE
         // is set (3.3 honored it): run spotread behind a pty via script(1)
         // instead — it then behaves interactively, streams line by line and
         // reads our keys raw.
         p.StartInfo.FileName = "script";
         p.StartInfo.ArgumentList.Add("-qefc");
         p.StartInfo.ArgumentList.Add($"{exe}{SpotReadArgs}");
         p.StartInfo.ArgumentList.Add("/dev/null");
      }

      p.ErrorDataReceived += ArgyllOutputHandler;
      p.OutputDataReceived += ArgyllOutputHandler;

      p.Start();
      _current = p;
      p.BeginErrorReadLine();
      p.BeginOutputReadLine();

      // instrument enumeration and init, possibly a user-assisted calibration:
      // no fixed timeout, just abort- and death-aware waiting
      while (!_ready.Wait(1000))
      {
         if (_abort || p.HasExited)
         {
            if (sw is not null)
               Console.Error.WriteLine(
                  $"PERF {DateTime.Now:HH:mm:ss.fff} spotread session failed after {sw.ElapsedMilliseconds} ms");
            return false;
         }

         // pending calibration: keep poking the sensor check — spotread only
         // re-tests the dial position on an input event, so an unanswered
         // prompt waits forever if the user rotates without pressing anything.
         // A space, not 'k': keys queued while the calibration measures are
         // drained at the menu, where 'k' would start another calibration.
         if (_calibrating) ArgyllSendKey(p, " ");
      }

      if (sw is not null)
         Console.Error.WriteLine(
            $"PERF {DateTime.Now:HH:mm:ss.fff} spotread session ready = {sw.ElapsedMilliseconds} ms");

      return true;
   }

   public bool SpotRead()
   {
      if (!Installed) return false;

      var sw = PerfTrace ? Stopwatch.StartNew() : null;

      while (!_abort)
      {
         if (!EnsureSession()) return false;

         _result.Reset();
         var menuSeq = Volatile.Read(ref _menuSeq);
         ArgyllSendKey(_current, "0");

         var waited = 0;
         var deadAir = 0;
         while (!_abort)
         {
            if (_result.Wait(500))
            {
               if (sw is not null)
                  Console.Error.WriteLine(
                     $"PERF {DateTime.Now:HH:mm:ss.fff} spotread reading = {sw.ElapsedMilliseconds} ms");
               return true;
            }

            var running = _current;
            if (running is null || running.HasExited) break; // session died: restart it

            // a calibration interposed itself before our reading: poke the
            // sensor check, spotread only re-tests the dial on input events
            // (space: harmless at the menu, 'k' there would recalibrate)
            waited++;
            if (_calibrating)
            {
               deadAir = 0; // waiting on the user, not on spotread
               if (waited % 4 == 0) ArgyllSendKey(running, " ");
            }
            // dead air with no calibration in progress: the state machine got
            // desynchronized (a prompt was consumed by a queued key, output we
            // don't recognize…) — a respawn costs one enumeration, an infinite
            // hang costs the whole run. Threshold above the slowest legitimate
            // reading (adaptive mode on dark patches takes tens of seconds).
            else if (++deadAir >= 120 && Volatile.Read(ref _menuSeq) == menuSeq)
            {
               InvalidateSession();
               break;
            }

            if (Volatile.Read(ref _menuSeq) != menuSeq)
            {
               // the menu came back without a result: reading refused (sensor
               // in the wrong position, transient instrument error) — pace the
               // retry, the wrong-position message is already displayed
               Thread.Sleep(1000);
               break;
            }
         }
      }

      return false;
   }

   public ProbedColor ProbedColor => new ProbedColorXYZ
   {
      Illuminant = ProbedColor.DIlluminant(ColorTemp),
      X = _xyz[0],
      Y = _xyz[1],
      Z = _xyz[2]
   };

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

