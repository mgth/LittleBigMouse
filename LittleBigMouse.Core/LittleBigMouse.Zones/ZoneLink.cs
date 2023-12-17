using System.Text.Json.Serialization;

namespace LittleBigMouse.Zoning;

public class ZoneLink : IZonesSerializable
{
    public double Distance { get; set; }
    public double From { get; set; }
    public double To { get; set; }
    public int SourceFromPixel { get; set; }
    public int SourceToPixel { get; set; }
    public int TargetFromPixel { get; set; }
    public int TargetToPixel { get; set; }
    [JsonIgnore] public Zone? Target { get; set; }
    public int TargetId => Target?.Id??-1;

    public string Serialize()
    {
        return ZoneSerializer.Serialize(this, 
            e => e.From, 
            e => e.To, 
            e => e.SourceFromPixel, 
            e => e.SourceToPixel, 
            e => e.TargetFromPixel, 
            e => e.TargetToPixel, 
           
            e => e.TargetId );
    }
}