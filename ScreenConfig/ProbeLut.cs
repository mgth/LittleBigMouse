using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Remoting.Messaging;
using System.Text;
using System.Threading.Tasks;
using System.Xml;
using System.Xml.Serialization;
using NotifyChange;

namespace LbmScreenConfig
{

    public class Tune
    {
        [XmlAttribute]
        public double Y;
        [XmlAttribute]
        public double x;
        [XmlAttribute]
        public double y;
        [XmlAttribute]
        public double Brightness;
        [XmlAttribute]
        public double Contrast;
        [XmlAttribute]
        public double Red;
        [XmlAttribute]
        public double Green;
        [XmlAttribute]
        public double Blue;
    }
        public static class ProbeLutExpendScreen
        {
            private static Dictionary<Screen, ProbeLut> _allLut = new Dictionary<Screen, ProbeLut>();
            public static ProbeLut ProbeLut(this Screen screen)
            {
                if (_allLut.ContainsKey(screen)) return _allLut[screen];

                ProbeLut lut = new ProbeLut(screen);
                _allLut.Add(screen, lut);
                return lut;
            }
    }


   // [assembly:InternalsVisibleTo("ProbeLutExpendScreen")]
    public class ProbeLut : INotifyPropertyChanged
    {
        // PropertyChanged Handling
        protected readonly PropertyChangedHelper Change;
        public event PropertyChangedEventHandler PropertyChanged { add { Change.Add(this, value); } remove { Change.Remove(value); } }
        public ProbedColor DIlluminant { get; }

        private readonly Screen _screen;

        private List<Tune> _lut = new List<Tune>();

        internal ProbeLut(Screen screen)
        {
            Change = new PropertyChangedHelper(this);
            _screen = screen;
            Change.Watch(Vcp, "VCP");
            Change.Watch(Vcp.Brightness, "Brightness");
        }

        public ScreenVcp Vcp => _screen.Vcp();

        public bool RemoveBrightness(double brightness)
        {
            Tune t = _lut.FirstOrDefault(x => x.Brightness == brightness);
            if (t == null) return false;

            _lut.Remove(t);
            return true;
        }

        public void Add(Tune tune)
        {
            _lut.Add(tune);

            _lut = _lut.OrderBy(x => x.Y).ToList();
        }

        public Tune FromLuminance(double luminance)
        {
            if (_lut.Count == 0) return Current;

            Tune tSup = null;
            Tune tInf = null;

            int i = 0;
            for (;i<_lut.Count &&  _lut[i].Y < luminance; i++)
                tInf = _lut[i];

            // luminance is more than monitor capabilities
            if (i >= _lut.Count) return tInf;

            tSup = _lut[i];

            if (tInf == null) return tSup;

            double dist = tSup.Y - tInf.Y;
            double ratio = (luminance - tInf.Y) / dist;

            Tune t = new Tune
            {
                Y = (uint)Math.Round(tInf.Y + (tSup.Y - tInf.Y) * ratio, 0),
                x = (uint)Math.Round(tInf.x + (tSup.x - tInf.x) * ratio, 0),
                y = (uint)Math.Round(tInf.y + (tSup.y - tInf.y) * ratio, 0),

                Brightness = (uint)Math.Round(tInf.Brightness + (tSup.Brightness - tInf.Brightness) * ratio, 0),
                Contrast = (uint)Math.Round(tInf.Contrast + (tSup.Contrast - tInf.Contrast) * ratio, 0),

                Red = (uint)Math.Round(tInf.Red + (tSup.Red - tInf.Red) * ratio, 0),
                Blue = (uint)Math.Round(tInf.Blue + (tSup.Blue - tInf.Blue) * ratio, 0),
                Green = (uint)Math.Round(tInf.Green + (tSup.Green - tInf.Green) * ratio, 0),
            };

            return t;
        }
        public Tune FromBrightness(double brightness)
        {
            if (_lut.Count == 0) return Current;

            Tune tSup = null;
            Tune tInf = null;

            int i = 0;
            for (; i < _lut.Count && _lut[i].Brightness < brightness; i++)
                tInf = _lut[i];

            // luminance is more than monitor capabilities
            if (i >= _lut.Count) return tInf;

            tSup = _lut[i];

            if (tInf == null) return tSup;

            double dist = tSup.Brightness - tInf.Brightness;
            double ratio = (brightness - tInf.Brightness) / dist;

            Tune t = new Tune
            {
                Y = (uint)Math.Round(tInf.Y + (tSup.Y - tInf.Y) * ratio, 0),
                x = (uint)Math.Round(tInf.x + (tSup.x - tInf.x) * ratio, 0),
                y = (uint)Math.Round(tInf.y + (tSup.y - tInf.y) * ratio, 0),

                Brightness = (uint)Math.Round(tInf.Brightness + (tSup.Brightness - tInf.Brightness) * ratio, 0),
                Contrast = (uint)Math.Round(tInf.Contrast + (tSup.Contrast - tInf.Contrast) * ratio, 0),

                Red = (uint)Math.Round(tInf.Red + (tSup.Red - tInf.Red) * ratio, 0),
                Blue = (uint)Math.Round(tInf.Blue + (tSup.Blue - tInf.Blue) * ratio, 0),
                Green = (uint)Math.Round(tInf.Green + (tSup.Green - tInf.Green) * ratio, 0),
            };

            return t;
        }

        private void SetLuminance(double luminance)
        {
            Tune t = FromLuminance(luminance);
            Vcp.Brightness.ValueAsync = (uint)Math.Round(t.Brightness,0);
            Vcp.Contrast.ValueAsync = (uint)Math.Round(t.Contrast, 0);
            Vcp.Gain.Red.ValueAsync = (uint)Math.Round(t.Red, 0);
            Vcp.Gain.Blue.ValueAsync = (uint)Math.Round(t.Blue, 0);
            Vcp.Gain.Green.ValueAsync = (uint)Math.Round(t.Green, 0);
        }

        public Tune Current => new Tune
        {
            Brightness = Vcp.Brightness.ValueAsync,
            Contrast = Vcp.Contrast.ValueAsync,
            Red = Vcp.Gain.Red.ValueAsync,
            Blue = Vcp.Gain.Blue.ValueAsync,
            Green = Vcp.Gain.Green.ValueAsync,
        };

        [DependsOn("Brightness.Value")]
        public double Luminance
        {
            get
            {
                Tune t = FromBrightness(Vcp.Brightness.ValueAsync);
                return t.Y;
            }
            set
            {
                SetLuminance(value);
            }
        }

        public Double MaxLuminance => 
            (_lut.Count==0)?1:_lut.Last().Y;

        public Double MinLuminance => 
            (_lut.Count==0)?0:_lut.First().Y;

        public string ConfigPath => Path.Combine(Vcp.Screen.ConfigPath(true), "Luminance.xml") ;

        public void Save()
        {
            XmlSerializer serializer = new XmlSerializer(typeof(List<Tune>));
            using (TextWriter writer = new StreamWriter(ConfigPath))
            {
                serializer.Serialize(writer, _lut);
            }
        }

        public void Load()
        {
            XmlSerializer deserializer = new XmlSerializer(typeof(List<Tune>));
            try
            {
                TextReader reader = new StreamReader(ConfigPath);
                _lut = (List<Tune>) deserializer.Deserialize(reader);
                reader.Close();
            }
            catch (FileNotFoundException)
            {
                
            }
        }
    }
}
