
using System.Collections.Concurrent;
using System.Text.Json.Serialization;
using Avalonia;

namespace LittleBigMouse.Zoning;

public class Zone : IZonesSerializable
{
    public int Id { get; set; }
    public string DeviceId { get; set; }
    public string Name { get; set; }

    public Rect PixelsBounds { get; set; }
    public Rect PhysicalBounds { get; set; }

    [JsonIgnore]
    public Zone Main { get; set;}

    public int MainId => Main.Id;

    public bool IsMain=> ReferenceEquals(this,Main);

    public double Dpi { get; private set; }

    Matrix _pixelsToPhysicalMatrix;
    Matrix _physicalToPixelsMatrix;

    public Zone(){}

    public Zone(
        string deviceId,
        string name,
        Rect pixelsBounds,
        Rect physicalBounds,
        Zone? main = null)
    {
        DeviceId = deviceId;
        Name = name;

        PixelsBounds = pixelsBounds;
        PhysicalBounds = physicalBounds;

        Main = main ?? this;
    }

    public void Init(int id)
    {
        Id = id;

        _pixelsToPhysicalMatrix = Matrix
            .CreateTranslation(PixelsBounds.X, -PixelsBounds.Y)
            .Append(Matrix.CreateScale(1 / PixelsBounds.Width, 1 / PixelsBounds.Height))
            .Append(Matrix.CreateScale(PhysicalBounds.Width, PhysicalBounds.Height))
            .Append(Matrix.CreateTranslation(PhysicalBounds.X, PhysicalBounds.Y));

        _physicalToPixelsMatrix = Matrix.CreateTranslation(-PhysicalBounds.X, -PhysicalBounds.Y)
            .Append(Matrix.CreateScale(1 / PhysicalBounds.Width, 1 / PhysicalBounds.Height))
            .Append(Matrix.CreateScale(PixelsBounds.Width, PixelsBounds.Height))
            .Append(Matrix.CreateTranslation(PixelsBounds.X, PixelsBounds.Y));

        var dpiX = PixelsBounds.Width / (PhysicalBounds.Width / 25.4);
        var dpiY = PixelsBounds.Height / (PhysicalBounds.Height / 25.4);

        Dpi = Math.Sqrt(dpiX * dpiX + dpiY * dpiY) / Math.Sqrt(2);
    }

    List<ZoneLink> ComputeLinks(
        ZonesLayout layout, 
        Func<Zone,double> nearFunc,  
        Func<Zone,double> farFunc,  
        Func<Zone,double> fromFunc, 
        Func<Zone,double> toFunc, 
        Func<Zone,int> fromPixelsFunc, 
        Func<Zone,int> toPixelsFunc, 
        double direction)
    {
        var values = new List<double> {double.MinValue, double.MaxValue};

        foreach (var zone in layout.Zones)
        {
            if (ReferenceEquals(zone, this)) continue;
            Add(fromFunc(zone));
            Add(toFunc(zone));
        }

        var links = new List<ZoneLink>();

        values = values.OrderBy(e => e).ToList();

        for (var i = 0; i < values.Count-1; i++)
        {
            var from = values[i];
            var to = values[i+ 1];
            Zone? target = null;
            var min = layout.MaxTravelDistance;


            //test if this zone is in or intersect current range 
            if (from <= toFunc(this) && to >= fromFunc(this))
            {
                foreach (var nextZone in layout.Zones.Except([this]))
                {
                    //test if next zone is in the right direction
                    if (direction * (farFunc(nextZone) - farFunc(this)) < 0) continue;

                    // test if next zone is in or intersect current range
                    if (from < fromFunc(nextZone) || to > toFunc(nextZone)) continue;

                    var distance = direction * (nearFunc(nextZone) - farFunc(this));

                    // test if next zone is nearer than best candidate
                    if (distance > min) continue;

                    target = nextZone;
                    min = distance;
                }

                target = target?.Main;
            }

            var sourceFrom = fromFunc(this);
            var sourceTo = toFunc(this);
            var targetFrom = target!=null?fromFunc(target):0.0;
            var targetTo = target!=null?toFunc(target):0.0;

            var sourceFromPixel = fromPixelsFunc(this);
            var sourceToPixel = toPixelsFunc(this);
            var targetFromPixel = target!=null?fromPixelsFunc(target):0;
            var targetToPixel = target!=null?toPixelsFunc(target):0;

            if (links.Count > 0 && ReferenceEquals(links.Last().Target, target))
            {
                links.Last().To = to;
                links.Last().SourceToPixel = Interpolate(to,sourceFrom,sourceTo, sourceFromPixel,sourceToPixel);
                links.Last().TargetToPixel = Interpolate(to,targetFrom,targetTo, targetFromPixel,targetToPixel);
            }
            else
            {

                links.Add(new ZoneLink
                {
                    Distance = min,
                    From = from,
                    To = to,
                    SourceFromPixel = Interpolate(from,sourceFrom,sourceTo, sourceFromPixel,sourceToPixel),
                    SourceToPixel = Interpolate(to,sourceFrom,sourceTo, sourceFromPixel,sourceToPixel),
                    TargetFromPixel = Interpolate(from,targetFrom,targetTo, targetFromPixel,targetToPixel),
                    TargetToPixel = Interpolate(to,targetFrom,targetTo, targetFromPixel,targetToPixel),
                    Target = target
                });
            }

            continue;

            int Interpolate(double value, double fromMm, double toMm, int pixelFrom, int pixelTo)
            {
                switch (value)
                {
                    case >= double.MaxValue:
                        return int.MaxValue;
                    case <= double.MinValue:
                        return int.MinValue;
                }

                var length = toMm - fromMm;
                var pixelLength = pixelTo - pixelFrom;

                return (int)((value - fromMm) * (double)pixelLength / length) + pixelFrom;
            }
        }

        if (links.Count==0) links.Add(new ZoneLink
        {
            Distance = double.MaxValue,

            From = double.MinValue, //fromFunc(this), TODO : 
            To = double.MaxValue, //toFunc(this),
            Target = null
        });

        return links;

        void Add(double v)
        {
            if(!values.Contains(v)) values.Add(v);
        }
    }

    public List<ZoneLink> LeftLinks { get; private set; }
    public List<ZoneLink> TopLinks { get; private set; }
    public List<ZoneLink> RightLinks { get; private set; }
    public List<ZoneLink> BottomLinks { get; private set; }

    public void ComputeLinks(ZonesLayout layout)
    {
        LeftLinks = ComputeLinks(layout, 
            z => z.PhysicalBounds.Right, 
            z => z.PhysicalBounds.Left, 
            z=>z.PhysicalBounds.Top,
            z => z.PhysicalBounds.Bottom, 
            z=> (int)z.PixelsBounds.Top,
            z => (int)z.PixelsBounds.Bottom, 
            -1);

        TopLinks = ComputeLinks(layout, 
            z => z.PhysicalBounds.Bottom, 
            z => z.PhysicalBounds.Top, 
            z=>z.PhysicalBounds.Left,
            z => z.PhysicalBounds.Right, 
            z=> (int)z.PixelsBounds.Left,
            z => (int)z.PixelsBounds.Right, 
            -1);

        RightLinks = ComputeLinks(layout, 
            z => z.PhysicalBounds.Left, 
            z => z.PhysicalBounds.Right, 
            z=>z.PhysicalBounds.Top,
            z => z.PhysicalBounds.Bottom, 
            z=>(int)z.PixelsBounds.Top,
            z => (int)z.PixelsBounds.Bottom, 
            
            1);

        BottomLinks = ComputeLinks(layout, 
            z => z.PhysicalBounds.Top, 
            z => z.PhysicalBounds.Bottom, 
            z=>z.PhysicalBounds.Left,
            z => z.PhysicalBounds.Right, 
            z=>(int)z.PixelsBounds.Left,
            z => (int)z.PixelsBounds.Right, 
            1);
    }


    public Point PixelsToPhysical(Point px) => px * _pixelsToPhysicalMatrix;

    public Point PhysicalToPixels(Point mm) => mm * _physicalToPixelsMatrix;

    public Point CenterPixel => new Point(PixelsBounds.Left + PixelsBounds.Width / 2, PixelsBounds.Top + PixelsBounds.Height / 2);

    public bool ContainsPixel(Point pixel)
    {
        if (pixel.X < PixelsBounds.X) return false;
        if (pixel.Y < PixelsBounds.Y) return false;
        if (pixel.X >= PixelsBounds.Right) return false;
        if (pixel.Y >= PixelsBounds.Bottom) return false;
        return true;
    }

    public bool ContainsMm(Point mm) => PhysicalBounds.Contains(mm);


    public Point InsidePixelsBounds(Point p)
    {
        if (p.X < PixelsBounds.X) p = new Point(PixelsBounds.X, p.Y);
        else if (p.X > PixelsBounds.Right - 1.0) p = new Point(PixelsBounds.Right - 1.0, p.Y);

        if (p.Y < PixelsBounds.Y) p = new Point(p.X, PixelsBounds.Y);
        else if (p.Y > PixelsBounds.Bottom - 1.0) p = new Point(p.X, PixelsBounds.Bottom - 1.0);

        return p;
    }

    public Point InsidePhysicalBounds(Point mm)
    {
        if (mm.X < PhysicalBounds.X) mm = new Point(PhysicalBounds.X, mm.Y);
        else if (mm.X > PhysicalBounds.Right) mm = new Point(PhysicalBounds.Right, mm.Y);

        if (mm.Y < PhysicalBounds.Y) mm = new Point(mm.X, PhysicalBounds.Y);
        else if (mm.Y > PhysicalBounds.Bottom) mm = new Point(mm.X, PhysicalBounds.Bottom);

        return mm;
    }

    readonly ConcurrentDictionary<Zone, IEnumerable<Rect>> _travels = new();

    public IEnumerable<Rect> TravelPixels(IEnumerable<Zone> zones, Zone target)
    {
        return _travels.GetOrAdd(target.Main, z => PixelsBounds.TravelPixels(z.PixelsBounds, zones.Where(z => ReferenceEquals(z, z.Main)).Select(z => z.PixelsBounds).ToArray()));
    }

    public string Serialize()
    {
        return ZoneSerializer.Serialize(this, e => e.Id, e => e.Name, 
            e => e.PixelsBounds, e => e.PhysicalBounds, 
            e => e.LeftLinks, e => e.TopLinks,
            e => e.RightLinks, e => e.BottomLinks);

        //return $@"<Zone Name=""{Name}"" DeviceId=""{DeviceId}""><PixelsBounds>{XmlSerializer.Serialize(PixelsBounds)}</PixelsBounds><PhysicalBounds>{XmlSerializer.Serialize(PhysicalBounds)}</PhysicalBounds></Zone>";
    }
}