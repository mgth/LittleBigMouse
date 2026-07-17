using System;
using System.Linq;
using System.Xml.Serialization;

namespace HLab.Sys.Windows.MonitorVcp;

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
    [XmlAttribute]
    public DateTime Date;

    /// <summary>ΔE00 against the target illuminant after white point tuning; 0 on entries predating the field.</summary>
    [XmlAttribute]
    public double DeltaE;

    public double MaxGain => new[]{Red, Green, Blue}.Max();
    public double MinGain => new[] { Red, Green, Blue }.Min();
}