
using System.Collections.Concurrent;
using System.Windows;
using System.Windows.Media;

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

        private Matrix _pixelsToPhysicalMatrix;
        private Matrix _physicalToPixelsMatrix;

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
            var pixelToPhysicalMatrix = new Matrix();
            pixelToPhysicalMatrix.Translate(-PixelsBounds.X, -PixelsBounds.Y);
            pixelToPhysicalMatrix.Scale(1 / PixelsBounds.Width, 1 / PixelsBounds.Height);
            pixelToPhysicalMatrix.Scale(PhysicalBounds.Width, PhysicalBounds.Height);
            pixelToPhysicalMatrix.Translate(PhysicalBounds.X, PhysicalBounds.Y);
            _pixelsToPhysicalMatrix = pixelToPhysicalMatrix;

            var physicalToPixelsMatrix = new Matrix();
            physicalToPixelsMatrix.Translate(-PhysicalBounds.X, -PhysicalBounds.Y);
            physicalToPixelsMatrix.Scale(1 / PhysicalBounds.Width, 1 / PhysicalBounds.Height);
            physicalToPixelsMatrix.Scale(PixelsBounds.Width, PixelsBounds.Height);
            physicalToPixelsMatrix.Translate(PixelsBounds.X, PixelsBounds.Y);
            _physicalToPixelsMatrix = physicalToPixelsMatrix;

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


        public Point InsidePixelsBounds(Point px)
        {
            if (px.X < PixelsBounds.X) px.X = PixelsBounds.X;
            else if (px.X > PixelsBounds.Right - 1.0) px.X = PixelsBounds.Right - 1.0;

            if (px.Y < PixelsBounds.Y) px.Y = PixelsBounds.Y;
            else if (px.Y > PixelsBounds.Bottom - 1.0) px.Y = PixelsBounds.Bottom - 1.0;

            return px;
        }

        public Point InsidePhysicalBounds(Point mm)
        {
            if (mm.X < PhysicalBounds.X) mm.X = PhysicalBounds.X;
            else if (mm.X > PhysicalBounds.Right) mm.X = PhysicalBounds.Right;

            if (mm.Y < PhysicalBounds.Y) mm.Y = PhysicalBounds.Y;
            else if (mm.Y > PhysicalBounds.Bottom) mm.Y = PhysicalBounds.Bottom;

            return mm;
        }

        private readonly ConcurrentDictionary<Zone, IEnumerable<Rect>> _travels = new();

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
