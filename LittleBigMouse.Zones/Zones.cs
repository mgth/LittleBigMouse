
using Newtonsoft.Json;

using System.Collections;
using System.Windows;

namespace LittleBigMouse.Zoning
{
    public class ZonesLayout
    {
        public bool AdjustPointer {get;set;}
        public bool AdjustSpeed {get;set;}

        public Zone FromPixel(Point pixel) => MainZones.FirstOrDefault(zone => zone.ContainsPixel(pixel));
        public Zone FromPhysical(Point physical) => Zones.FirstOrDefault(zone => zone.ContainsMm(physical));

        [JsonProperty(ItemIsReference = true)]
        public List<Zone> Zones {get;} = new();

        [JsonProperty(ItemIsReference = true)]
        public List<Zone> MainZones {get;} = new();

        public void Init()
        {
            MainZones.Clear();
            MainZones.AddRange(Zones.Where(z => z.IsMain));

            foreach (var zone in Zones) zone.Init();
        }

    }
}
