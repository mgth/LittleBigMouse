using System;
using System.Collections.Generic;
using System.Linq;
using Avalonia.Styling;
using ReactiveUI;

namespace HLab.Sys.Windows.MonitorVcp;

public class MonitorRgbLevel : ReactiveObject, IDisposable
{
   readonly MonitorLevel[] _values = new MonitorLevel[3];

   public MonitorRgbLevel(CommandWorker parser, VcpGetter getter, VcpSetter setter)
   {
      for (var i = 0; i < 3; i++)
         _values[i] = new MonitorLevel(parser, getter, setter, (VcpComponent)i);
   }

   public MonitorRgbLevel Start()
   {
      foreach (var level in _values)
         level.Start();

      return this;
   }

   public MonitorLevel Channel(uint channel) { return _values[channel]; }

   public MonitorLevel Red => Channel(0);
   public MonitorLevel Green => Channel(1);
   public MonitorLevel Blue => Channel(2);

   public void SetToMax()
   {
      foreach (var level in _values)
         level.SetToMax();
   }

   public void SetToMin()
   {
      foreach (var level in _values)
         level.SetToMin();
   }

   public void SetTo(uint[] value)
   {
      for (var i = 0; i < value.Length; i++)
      {
         if(_values.Length <= i) break;
         _values[i].Value = value[i];
      }
   }

   public uint[] GetValues() => _values.Select(t => t.Value).ToArray();

   public void Dispose()
   {
      foreach (var level in _values) level.Dispose();
      GC.SuppressFinalize(this);
   }
}
