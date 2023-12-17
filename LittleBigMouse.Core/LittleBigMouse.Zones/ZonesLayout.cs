using System.Text.Json.Serialization;
using Avalonia;

namespace LittleBigMouse.Zoning
{
    public class ZonesLayout : IZonesSerializable
    {
        public bool AdjustPointer {get;set;}
        public bool AdjustSpeed {get;set;}
        public bool LoopX {get;set;}
        public bool LoopY {get;set;}
        public string Priority {get; set;}

        public string Algorithm { get; set; } = "strait";
        public double MaxTravelDistance { get; set; } = 200.0;

        public Zone FromPixel(Point pixel) => MainZones.FirstOrDefault(zone => zone.ContainsPixel(pixel));
        public Zone FromPhysical(Point physical) => Zones.FirstOrDefault(zone => zone.ContainsMm(physical));

        public List<Zone> Zones {get;} = new();

        [JsonIgnore]
        public List<Zone> MainZones {get;} = new();

        public void Init()
        {
            MainZones.Clear();
            MainZones.AddRange(Zones.Where(z => z.IsMain));

            for (var i = 0; i<Zones.Count; i++)
            {
                Zones[i].Init(i);

                if(Zones[i].IsMain)
                    Zones[i].ComputeLinks(this);
            }
        }

        public string Serialize()
        {
            return ZoneSerializer.Serialize(this,
                e => e.AdjustPointer, 
                e => e.AdjustSpeed, 
                e => e.LoopX,
                e => e.LoopY,
                e => e.Priority,
                e => e.Algorithm,
                e => e.MaxTravelDistance,
                e => e.MainZones);
        }
    }
}
