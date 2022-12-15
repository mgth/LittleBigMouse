
using Newtonsoft.Json;

using System.Collections;
using System.Runtime.Serialization;
using System.Windows;
using Avalonia;

namespace LittleBigMouse.Zoning
{
    public class ZonesLayout : IXmlSerializable
    {
        public bool AdjustPointer {get;set;}
        public bool AdjustSpeed {get;set;}

        public Zone FromPixel(Point pixel) => MainZones.FirstOrDefault(zone => zone.ContainsPixel(pixel));
        public Zone FromPhysical(Point physical) => Zones.FirstOrDefault(zone => zone.ContainsMm(physical));

        public List<Zone> Zones {get;} = new();

        public List<Zone> MainZones {get;} = new();

        public void Init()
        {
            MainZones.Clear();
            MainZones.AddRange(Zones.Where(z => z.IsMain));

            foreach (var zone in Zones) zone.Init();
        }

        public string Serialize()
        {
            return XmlSerializer.Serialize(this,e => e.AdjustPointer, e => e.AdjustSpeed, e=> e.Zones);
        }
    }
}
