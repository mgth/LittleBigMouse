using System;
using HLab.Sys.Monitors;
using System.Collections.Generic;
using System.Linq;
using System.Xml.Serialization;

namespace HLab.Sys.Windows.Monitors;

public class MonitorDevice : IEquatable<MonitorDevice>
{
   public string Id { get; init; } = "";
   public string PnpCode { get; init; } = "";
   public string PhysicalId { get; set; } = "";
   public string SourceId { get; set; } = "";

   /// <summary>
   /// Device interface path (\\?\DISPLAY#...) identifying this monitor for the
   /// CCD API, independently of the adapter source it is (or was) bound to.
   /// </summary>
   public string InterfacePath { get; set; } = "";
   public Edid Edid { get; set; }
   public string MonitorNumber { get; set; } = "";

   /// <summary>
   /// Specialized display (VR headset like Windows Mixed Reality...): driven by a
   /// dedicated runtime and hidden from the desktop by Windows.
   /// </summary>
   public bool IsSpecialized { get; set; }

   [XmlIgnore]
   public List<MonitorDeviceConnection> Connections = new();

   /// <summary>
   /// Connection currently participating in the desktop. EnumDisplayDevices can
   /// expose the same target below several adapters, including stale inactive
   /// candidates, so enumeration order is not an identity.
   /// </summary>
   [XmlIgnore]
   public MonitorDeviceConnection? ActiveConnection => SelectConnection(Connections);

   public static MonitorDeviceConnection? SelectConnection(
      IEnumerable<MonitorDeviceConnection> connections)
      => connections
         .OrderByDescending(c => c.State?.AttachedToDesktop == true)
         .ThenByDescending(c => c.Parent.HMonitor != 0)
         .ThenByDescending(c => c.Parent.CurrentMode != null)
         .ThenBy(c => c.Parent.DeviceName, StringComparer.OrdinalIgnoreCase)
         .FirstOrDefault();

   public override bool Equals(object obj)
   {
      if (obj is MonitorDevice other) return Id == other.Id;
      return base.Equals(obj);
   }

   public override int GetHashCode() => HashCode.Combine(Id);

   public bool Equals(MonitorDevice other) => Id == other.Id;
}
