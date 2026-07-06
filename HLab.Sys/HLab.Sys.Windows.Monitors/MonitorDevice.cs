using System;
using System.Collections.Generic;
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

   public override bool Equals(object obj)
   {
      if (obj is MonitorDevice other) return Id == other.Id;
      return base.Equals(obj);
   }

   public override int GetHashCode() => HashCode.Combine(Id);

   public bool Equals(MonitorDevice other) => Id == other.Id;
}