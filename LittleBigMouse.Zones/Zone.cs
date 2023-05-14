
using System.Collections.Concurrent;
using Avalonia;

namespace LittleBigMouse.Zoning
{
    public class Zone : IXmlSerializable
    {
        public string DeviceId { get; set; }
        public string Name { get; set; }

        public Rect PixelsBounds { get; set; }
        public Rect PhysicalBounds { get; set; }

        public Zone Main { get; set;}

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

            Init();
        }

        public void Init()
        {
            _pixelsToPhysicalMatrix = Matrix
                    .CreateTranslation(-PixelsBounds.X, -PixelsBounds.Y)
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
            return XmlSerializer.Serialize(this, e => e.Name, /*e => e.DeviceId,*/ e => e.PixelsBounds, e => e.PhysicalBounds );
            //return $@"<Zone Name=""{Name}"" DeviceId=""{DeviceId}""><PixelsBounds>{XmlSerializer.Serialize(PixelsBounds)}</PixelsBounds><PhysicalBounds>{XmlSerializer.Serialize(PhysicalBounds)}</PhysicalBounds></Zone>";
        }
    }
}
