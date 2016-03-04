using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Xml.Serialization;
using Argyll;
using LbmScreenConfig;
using NotifyChange;

namespace MonitorVcp
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

        public double MaxGain => (new double[]{Red, Green, Blue}).Max();
        public double MinGain => (new double[] { Red, Green, Blue }).Min();
    }
        public static class ProbeLutExpendScreen
        {
            private static readonly Dictionary<Screen, ProbeLut> AllLut = new Dictionary<Screen, ProbeLut>();
            public static ProbeLut ProbeLut(this Screen screen)
            {
                if (AllLut.ContainsKey(screen)) return AllLut[screen];

                ProbeLut lut = new ProbeLut(screen);
                AllLut.Add(screen, lut);
                return lut;
            }
    }


   // [assembly:InternalsVisibleTo("ProbeLutExpendScreen")]
    public class ProbeLut : Notifier
    {
        public ProbedColor DIlluminant { get; }

        private readonly Screen _screen;

        private List<Tune> _lut = new List<Tune>();

        internal ProbeLut(Screen screen)
        {
            _screen = screen;
            Watch(Vcp, "Vcp");
            Watch(Vcp.Brightness, "Brightness");
        }

        public MonitorVcp Vcp => _screen.Monitor.Vcp();

        public bool RemoveBrightness(double brightness)
        {
            Tune t = _lut.FirstOrDefault(x => x.Brightness == brightness);
            if (t == null) return false;

            _lut.Remove(t);
            return true;
        }
        public bool RemoveLowBrightness(double maxgain)
        {
            Tune t = _lut.FirstOrDefault(x => (x.Brightness == 0 && x.MaxGain== maxgain));
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
            Vcp.Brightness.Value = (uint)Math.Round(t.Brightness,0);
            Vcp.Contrast.Value = (uint)Math.Round(t.Contrast, 0);
            Vcp.Gain.Red.Value = (uint)Math.Round(t.Red, 0);
            Vcp.Gain.Blue.Value = (uint)Math.Round(t.Blue, 0);
            Vcp.Gain.Green.Value = (uint)Math.Round(t.Green, 0);
        }

        public Tune Current => new Tune
        {
            Brightness = Vcp.Brightness.Value,
            Contrast = Vcp.Contrast.Value,
            Red = Vcp.Gain.Red.Value,
            Blue = Vcp.Gain.Blue.Value,
            Green = Vcp.Gain.Green.Value,
        };

        [DependsOn("Brightness.Value")]
        public double Luminance
        {
            get
            {
                Tune t = FromBrightness(Vcp.Brightness.Value);
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

        public string ConfigPath => Path.Combine(_screen.ConfigPath(true), "Luminance.xml") ;

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
                _lut = new List<Tune>
                {
                    new Tune
                    {
                        Brightness = MinLuminance,
                        Y = 0,
                        Red = Vcp.Gain.Red.Value,
                        Blue = Vcp.Gain.Blue.Value,
                        Green = Vcp.Gain.Blue.Value,
                        Contrast = Vcp.Contrast.Value
                    },
                    new Tune
                    {
                        Brightness = MaxLuminance,
                        Y = 160,
                        Red = Vcp.Gain.Red.Value,
                        Blue = Vcp.Gain.Blue.Value,
                        Green = Vcp.Gain.Blue.Value,
                        Contrast = Vcp.Contrast.Value
                    },
                };
            }
        }
    }
}
