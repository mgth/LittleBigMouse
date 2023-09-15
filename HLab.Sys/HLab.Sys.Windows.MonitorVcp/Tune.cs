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

    public double MaxGain => new[]{Red, Green, Blue}.Max();
    public double MinGain => new[] { Red, Green, Blue }.Min();
}