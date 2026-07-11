/*
  HLab.Windows.MonitorVcp
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.MonitorVcp.

    HLab.Windows.MonitorVcp is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.MonitorVcp is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

#nullable enable
using System;
using System.Collections.Generic;
using System.Reactive.Concurrency;
using System.Threading.Tasks;
using ReactiveUI;
using OneOf;

using static HLab.Sys.Windows.API.MonitorConfiguration.LowLevelMonitorConfiguration;

namespace HLab.Sys.Windows.MonitorVcp;

public enum VcpComponent
{
   None = -1,
   Red = 0,
   Green = 1,
   Blue = 2,
   Brightness = -2,
   Contrast = -3
}


public class VcpControl : ReactiveObject
{
   readonly IVcpTransport _transport;
   readonly CommandWorker _worker;

   /// <summary>Identity of the monitor this channel talks to (used for the LUT store).</summary>
   public string MonitorId { get; }

   /// <summary>EDID PnP manufacturer code ("DEL", "PHL"…), drives quirks like AlternatePower.</summary>
   public string? ManufacturerCode { get; }

   public VcpControl(string monitorId, string? manufacturerCode, IVcpTransport transport, CommandWorker worker)
   {
      MonitorId = monitorId;
      ManufacturerCode = manufacturerCode;
      _transport = transport;
      _worker = worker;

      Source = new MonitorLevel(worker, GetSource, SetSource);

      // The capabilities probe is a full DDC transaction — seconds per monitor
      // when the bus is slow or the monitor asleep. Run it off-thread; the
      // levels pop in through their reactive properties when it lands.
      Task.Run(Probe);
   }

   void Probe()
   {
      IReadOnlySet<byte>? codes = null;
      try
      {
         var retry = 1;
         while (retry-- >= 0 && (codes = _transport.GetSupportedCodes()) is null) { }
      }
      finally
      {
         RxSchedulers.MainThreadScheduler.Schedule(() => OnProbed(codes));
      }
   }

   void OnProbed(IReadOnlySet<byte>? codes)
   {
      lock (_startLock)
      {
         if (codes is not null)
         {
            if (codes.Contains((byte)VcpCode.Brightness))
               Brightness ??= NewLevel(GetBrightness, SetBrightness);

            if (codes.Contains((byte)VcpCode.Contrast))
               Contrast ??= NewLevel(GetContrast, SetContrast);

            if (codes.Contains((byte)VcpCode.RedGain)
                && codes.Contains((byte)VcpCode.GreenGain)
                && codes.Contains((byte)VcpCode.BlueGain))
               Gain ??= NewRgbLevel(GetGain, SetGain);

            if (codes.Contains((byte)VcpCode.RedDrive)
                && codes.Contains((byte)VcpCode.GreenDrive)
                && codes.Contains((byte)VcpCode.BlueDrive))
               Drive ??= NewRgbLevel(GetDrive, SetDrive);
         }

         Probing = false;
      }
   }

   MonitorLevel NewLevel(VcpGetter getter, VcpSetter setter)
   {
      var level = new MonitorLevel(_worker, getter, setter);
      return _started ? level.Start() : level;
   }

   MonitorRgbLevel NewRgbLevel(VcpGetter getter, VcpSetter setter)
   {
      var level = new MonitorRgbLevel(_worker, getter, setter);
      return _started ? level.Start() : level;
   }

   readonly object _startLock = new();
   bool _started;

   public VcpControl Start()
   {
      lock (_startLock)
      {
         _started = true;
         _brightness?.Start();
         _contrast?.Start();
         _gain?.Start();
         _drive?.Start();
      }
      return this;
   }

   /// <summary>True while the background capabilities probe has not answered yet.</summary>
   public bool Probing
   {
      get => _probing;
      private set => this.RaiseAndSetIfChanged(ref _probing, value);
   }
   bool _probing = true;

   public bool AlternatePower => ManufacturerCode == "DEL";

   public bool Power
   {
      get => _power;
      set
      {
         if (value == _power) return;
         if (value)
         {
            if (AlternatePower)
               _transport.SetFeature(VcpCode.PowerAlternate, 0);
            else
               _transport.SetFeature(VcpCode.Power, 1);
         }
         else
         {
            if (AlternatePower)
               _transport.SetFeature(VcpCode.PowerAlternate, 1);
            else
               _transport.SetFeature(VcpCode.Power, 4);
         }
         this.RaiseAndSetIfChanged(ref _power, value);

      }
   }
   bool _power;


   public void ActivateAnyway()
   {
      Brightness ??= new MonitorLevel(_worker, GetBrightness, SetBrightness).Start();
      Contrast ??= new MonitorLevel(_worker, GetContrast, SetContrast).Start();

      Gain ??= new MonitorRgbLevel(_worker, GetGain, SetGain).Start();

      Source ??= new MonitorLevel(_worker, GetSource, SetSource).Start();

      // TODO : Drive seams to never work when not officially supported
      // Drive ??= new MonitorRgbLevel(_levelParser, GetDrive, SetDrive).Start();
   }
   public MonitorLevel? Source
   {
      get => _source;
      private set => this.RaiseAndSetIfChanged(ref _source, value);
   }
   MonitorLevel? _source = null;

   public MonitorLevel? Brightness
   {
      get => _brightness;
      private set => this.RaiseAndSetIfChanged(ref _brightness, value);
   }
   MonitorLevel? _brightness = null;

   public MonitorLevel? Contrast
   {
      get => _contrast;
      private set => this.RaiseAndSetIfChanged(ref _contrast, value);
   }
   MonitorLevel? _contrast = null;

   public MonitorRgbLevel? Gain
   {
      get => _gain;
      private set => this.RaiseAndSetIfChanged(ref _gain, value);
   }
   MonitorRgbLevel? _gain = null;

   public MonitorRgbLevel? Drive
   {
      get => _drive;
      private set => this.RaiseAndSetIfChanged(ref _drive, value);
   }
   MonitorRgbLevel? _drive = null;

   static VcpCode GainCode(VcpComponent component) => component switch
   {
      VcpComponent.Red => VcpCode.RedGain,
      VcpComponent.Green => VcpCode.GreenGain,
      VcpComponent.Blue => VcpCode.BlueGain,
      _ => throw new ArgumentOutOfRangeException(nameof(component), component, null)
   };

   static VcpCode DriveCode(VcpComponent component) => component switch
   {
      VcpComponent.Red => VcpCode.RedDrive,
      VcpComponent.Green => VcpCode.GreenDrive,
      VcpComponent.Blue => VcpCode.BlueDrive,
      _ => throw new ArgumentOutOfRangeException(nameof(component), component, null)
   };

   OneOf<(uint value, uint min, uint max), int> GetSource(VcpComponent component = VcpComponent.None)
      => _transport.GetFeature(VcpCode.InputSource);
   bool SetSource(uint value, VcpComponent component = VcpComponent.None)
      => _transport.SetFeature(VcpCode.InputSource, value);

   OneOf<(uint value, uint min, uint max), int> GetBrightness(VcpComponent component = VcpComponent.None)
      => _transport.GetFeature(VcpCode.Brightness);
   bool SetBrightness(uint value, VcpComponent component = VcpComponent.None)
      => _transport.SetFeature(VcpCode.Brightness, value);

   OneOf<(uint value, uint min, uint max), int> GetContrast(VcpComponent component = VcpComponent.None)
      => _transport.GetFeature(VcpCode.Contrast);
   bool SetContrast(uint value, VcpComponent component = VcpComponent.None)
      => _transport.SetFeature(VcpCode.Contrast, value);

   OneOf<(uint value, uint min, uint max), int> GetGain(VcpComponent component)
      => _transport.GetFeature(GainCode(component));
   bool SetGain(uint value, VcpComponent component = VcpComponent.None)
      => _transport.SetFeature(GainCode(component), value);

   OneOf<(uint value, uint min, uint max), int> GetDrive(VcpComponent component)
      => _transport.GetFeature(DriveCode(component));
   bool SetDrive(uint value, VcpComponent component)
      => _transport.SetFeature(DriveCode(component), value);
}

public delegate OneOf<(uint value, uint min, uint max), int> VcpGetter(VcpComponent component = VcpComponent.None);
public delegate bool VcpSetter(uint value, VcpComponent component = VcpComponent.None);
