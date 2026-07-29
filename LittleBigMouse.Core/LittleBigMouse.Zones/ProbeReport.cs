using System.Xml;
using System.Xml.Linq;

namespace LittleBigMouse.Zoning;

/// <summary>
/// One homogeneous stretch of a zone edge, as reported by the daemon's edge prober:
/// pixels <see cref="From"/>..<see cref="To"/> (inclusive, desktop coordinates along the
/// edge — y for Left/Right, x for Top/Bottom) all cross into zone <see cref="TargetId"/>,
/// or hit a wall (<see cref="TargetId"/> == -1).
/// </summary>
public sealed record ProbeRun(int From, int To, int TargetId)
{
    public bool IsWall => TargetId < 0;
}

public sealed record ProbeEdge(string Side, IReadOnlyList<ProbeRun> Runs);

public sealed record ProbeZone(int Id, string Name, string DeviceId, IReadOnlyList<ProbeEdge> Edges);

/// <summary>
/// Parsed <c>&lt;ProbeReport&gt;</c> document — the payload of a <see cref="LittleBigMouseEvent.Probed"/>
/// event. Produced by driving the daemon's REAL engine over every edge of every main zone,
/// so it reflects the configured algorithm, the loop options and the zone links exactly as
/// the cursor would experience them.
/// </summary>
public sealed class ProbeReport
{
    ProbeReport(string algorithm, bool loopX, bool loopY, bool isVirtual, IReadOnlyList<ProbeZone> zones)
    {
        Algorithm = algorithm;
        LoopX = loopX;
        LoopY = loopY;
        Virtual = isVirtual;
        Zones = zones;
    }

    public string Algorithm { get; }
    public bool LoopX { get; }
    public bool LoopY { get; }
    public bool Virtual { get; }
    public IReadOnlyList<ProbeZone> Zones { get; }

    const int MaxReportCharacters = 4 * 1024 * 1024;

    public static bool TryParse(string xml, out ProbeReport? report)
    {
        report = null;
        if (string.IsNullOrWhiteSpace(xml) || xml.Length > MaxReportCharacters) return false;

        try
        {
            using var text = new StringReader(xml);
            using var reader = XmlReader.Create(text, new XmlReaderSettings
            {
                DtdProcessing = DtdProcessing.Prohibit,
                MaxCharactersInDocument = MaxReportCharacters,
                XmlResolver = null,
            });
            var root = XDocument.Load(reader).Root;
            if (root?.Name.LocalName != "ProbeReport") return false;

            var zones = new List<ProbeZone>();
            foreach (var zoneEl in root.Elements("Zone"))
            {
                var edges = new List<ProbeEdge>();
                foreach (var edgeEl in zoneEl.Elements("Edge"))
                {
                    var runs = new List<ProbeRun>();
                    foreach (var runEl in edgeEl.Elements("Run"))
                    {
                        runs.Add(new ProbeRun(
                            GetInt(runEl, "From"),
                            GetInt(runEl, "To"),
                            GetInt(runEl, "Target")));
                    }
                    edges.Add(new ProbeEdge(edgeEl.Attribute("Side")?.Value ?? "", runs));
                }
                zones.Add(new ProbeZone(
                    GetInt(zoneEl, "Id"),
                    zoneEl.Attribute("Name")?.Value ?? "",
                    zoneEl.Attribute("DeviceId")?.Value ?? "",
                    edges));
            }

            report = new ProbeReport(
                root.Attribute("Algorithm")?.Value ?? "",
                GetBool(root, "LoopX"),
                GetBool(root, "LoopY"),
                GetBool(root, "Virtual"),
                zones);
            return true;
        }
        catch (XmlException)
        {
            return false;
        }
    }

    static int GetInt(XElement el, string name)
        => int.TryParse(el.Attribute(name)?.Value, out var value) ? value : -1;

    static bool GetBool(XElement el, string name)
        => string.Equals(el.Attribute(name)?.Value, "True", StringComparison.OrdinalIgnoreCase);
}
