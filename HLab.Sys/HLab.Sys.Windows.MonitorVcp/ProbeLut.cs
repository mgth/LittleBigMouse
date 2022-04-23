/*
  HLab.Windows.MonitorVcp
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.MonitorVcp.

    HLab.Windows.MonitorVcp is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.MonitorVcp is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Xml.Serialization;
using HLab.Sys.Argyll;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.Monitors;

namespace HLab.Sys.Windows.MonitorVcp
{
    using H = H<ProbeLut>;

    public class ProbeLut : NotifierBase
    {
        public ProbedColor DIlluminant { get; }

        private readonly MonitorDevice _monitor;

        private List<Tune> _lut = new List<Tune>();

        internal ProbeLut(MonitorDevice monitor)
        {
            _monitor = monitor;
            H.Initialize(this);
        }

        public VcpControl Vcp => _monitor.Vcp();

        public bool RemoveBrightness(double brightness)
        {
            var t = _lut.FirstOrDefault(x => x.Brightness == brightness);
            if (t == null) return false;

            _lut.Remove(t);
            return true;
        }
        public bool RemoveLowBrightness(double maxgain)
        {
            var t = _lut.FirstOrDefault(x => (x.Brightness == 0 && x.MaxGain== maxgain));
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

            var i = 0;
            for (;i<_lut.Count &&  _lut[i].Y < luminance; i++)
                tInf = _lut[i];

            // luminance is more than monitor capabilities
            if (i >= _lut.Count) return tInf;

            tSup = _lut[i];

            if (tInf == null) return tSup;

            var dist = tSup.Y - tInf.Y;
            var ratio = (luminance - tInf.Y) / dist;

            var t = new Tune
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

            var i = 0;
            for (; i < _lut.Count && _lut[i].Brightness < brightness; i++)
                tInf = _lut[i];

            // luminance is more than monitor capabilities
            if (i >= _lut.Count) return tInf;

            tSup = _lut[i];

            if (tInf == null) return tSup;

            var dist = tSup.Brightness - tInf.Brightness;
            var ratio = (brightness - tInf.Brightness) / dist;

            var t = new Tune
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

        private double GetLuminance() => FromBrightness(Vcp.Brightness.Value).Y;

        private void SetLuminance(double luminance)
        {
            var t = FromLuminance(luminance);
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

        public double Luminance
        {
            get => _luminance.Get();
            set => SetLuminance(value);
        }

        private readonly IProperty<double> _luminance = H.Property<double>(c => c
            .Set(e => e.GetLuminance())
            .On(e => e.Vcp.Brightness.Value)
            .Update()
        );

        public double MaxLuminance => 
            (_lut.Count==0)?1:_lut.Last().Y;

        public double MinLuminance => 
            (_lut.Count==0)?0:_lut.First().Y;

        public string ConfigPath => Path.Combine(_monitor.ConfigPath(true), "Luminance.xml") ;

        public void Save()
        {
            var serializer = new XmlSerializer(typeof(List<Tune>));
            using (TextWriter writer = new StreamWriter(ConfigPath))
            {
                serializer.Serialize(writer, _lut);
            }
        }

        public void Load()
        {
            var deserializer = new XmlSerializer(typeof(List<Tune>));
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
                        Red = Vcp.Gain?.Red.Value??0,
                        Blue = Vcp.Gain?.Blue.Value??0,
                        Green = Vcp.Gain?.Green.Value??0,
                        Contrast = Vcp.Contrast?.Value??0
                    },
                    new Tune
                    {
                        Brightness = MaxLuminance,
                        Y = 160,
                        Red = Vcp.Gain?.Red.Value??0,
                        Blue = Vcp.Gain?.Blue.Value??0,
                        Green = Vcp.Gain?.Green.Value??0,
                        Contrast = Vcp.Contrast?.Value??0
                    },
                };
            }
        }
    }
}
