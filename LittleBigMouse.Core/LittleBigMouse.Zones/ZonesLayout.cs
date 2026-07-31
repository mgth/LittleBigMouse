using HLab.Geo;
using System.Text.Json.Serialization;

namespace LittleBigMouse.Zoning
{
    public class ZonesLayout : IZonesSerializable
    {
        public bool AdjustPointer {get;set;}
        public bool AdjustSpeed {get;set;}
        public bool LoopX {get;set;}
        public bool LoopY {get;set;}

        /// <summary>
        /// True when these zones come from a virtual (foreign) layout. Serialized on the
        /// wire: the daemon refuses to hook a virtual layout no matter what commands
        /// follow, so a client's geometry can never capture the local mouse.
        /// </summary>
        public bool Virtual {get;set;}
        /// <summary>
        /// The panic shortcut, carried to the daemon like every other daemon-side
        /// setting — which is what gets it to a standalone daemon replaying its startup
        /// file. The daemon is what registers it; nothing here reads it back.
        /// </summary>
        public string RescueShortcut {get; set;} = "";

        public string Priority {get; set;}
        public string PriorityUnhooked {get; set;}

        public string Algorithm { get; set; } = "strait";
        public double MaxTravelDistance { get; set; } = 200.0;
        public double FreelookCheckInterval { get; set; } = 100.0;
        public bool FreelookEnabled { get; set; } = true;

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
                e => e.Virtual,
                e => e.RescueShortcut,
                e => e.Priority,
                e => e.PriorityUnhooked,
                e => e.Algorithm,
                e => e.MaxTravelDistance,
                e => e.FreelookCheckInterval,
                e => e.FreelookEnabled,
                e => e.MainZones);
        }
    }
}
