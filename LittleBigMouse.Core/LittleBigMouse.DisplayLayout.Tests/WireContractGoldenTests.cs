using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// Golden tests of the UI↔daemon wire contract, over the shared corpus in
/// <c>wire-contract/goldens</c>. The Rust hook's <c>tests/wire_goldens.rs</c> reads the
/// SAME files from the source tree, so a payload this side produces is provably the
/// payload that side parses — the two ends never compare against separate copies.
/// <para>
/// Direction matters, because authority differs by direction:
/// </para>
/// <list type="bullet">
/// <item><description>
/// UI→daemon (<c>ui-to-daemon/</c>): C# is the producer of record. The XML names are
/// not written anywhere — <see cref="ZoneSerializer"/> derives them by reflection from
/// the C# member names, so renaming a property here silently renames a wire attribute.
/// That is exactly what these goldens exist to catch.
/// </description></item>
/// <item><description>
/// daemon→UI (<c>daemon-to-ui/</c>): Rust is the producer of record. Those goldens are
/// regenerated on the Rust side; here they are only ever parsed.
/// </description></item>
/// </list>
/// <para>
/// Set <c>LBM_UPDATE_GOLDEN=1</c> to rewrite the goldens this side owns after an
/// INTENTIONAL format change, then read the diff before committing: a changed line is a
/// daemon that no longer understands what the UI sends. See
/// <c>wire-contract/README.md</c> for the full change procedure.
/// </para>
/// </summary>
public class WireContractGoldenTests
{
    //==========//
    // Corpus IO

    /// <summary>
    /// The shared corpus, resolved in the SOURCE tree rather than the build output: the
    /// point of these goldens is that both languages read the same bytes, and a copy in
    /// bin/ would let the two drift apart silently.
    /// </summary>
    static string GoldenDir
    {
        get
        {
            var dir = new DirectoryInfo(AppContext.BaseDirectory);
            while (dir is not null)
            {
                var candidate = Path.Combine(dir.FullName, "wire-contract", "goldens");
                if (Directory.Exists(candidate)) return candidate;
                dir = dir.Parent;
            }
            throw new DirectoryNotFoundException(
                "wire-contract/goldens not found above " + AppContext.BaseDirectory);
        }
    }

    static string ReadGolden(string relative) =>
        File.ReadAllText(Path.Combine(GoldenDir, relative)).Replace("\r\n", "\n").TrimEnd('\n');

    /// <summary>
    /// Compare against a golden this side OWNS (UI→daemon). Honours LBM_UPDATE_GOLDEN.
    /// </summary>
    static void AssertOwnedGolden(string relative, string actual)
    {
        var path = Path.Combine(GoldenDir, relative);

        if (Environment.GetEnvironmentVariable("LBM_UPDATE_GOLDEN") == "1")
            File.WriteAllText(path, actual + "\n");

        Assert.True(File.Exists(path), $"missing golden file: {relative}");
        Assert.Equal(ReadGolden(relative), actual);
    }

    //=======================//
    // The layout under test

    // Two 1920x1080 monitors of equal DPI, side by side, physically adjacent. Left's
    // right edge runs 0..270 mm and maps 1:1 onto Right's left edge. Deliberately small:
    // a golden nobody can read by eye is a golden nobody reviews.
    const double EdgeHeightMm = 270;

    /// <summary>
    /// The layout the current golden is built from. Every wire field that can vary is
    /// given a NON-DEFAULT value, so a golden that stops matching names the field: a
    /// default-valued field would still serialize if the code that sets it were dropped.
    /// </summary>
    static ZonesLayout CurrentLayout()
    {
        var leftBorders = new BorderResistance();

        // Two sections on Left's right edge: one plain resistance, one that blocks drags
        // only. Sections never reach the daemon as such — they are folded into the flat
        // ZoneLink list — so this is what exercises MoveBlock/DragResistance/DragBlock,
        // the three attributes v5.6 added.
        leftBorders.Right.Sections.Add(new BorderSection
        {
            From = 0, To = 135, Move = 12.5, Drag = 30
        });
        leftBorders.Right.Sections.Add(new BorderSection
        {
            From = 135, To = 270, Move = 12.5, DragBlock = true
        });

        var layout = new ZonesLayout
        {
            AdjustPointer = true,
            AdjustSpeed = false,
            LoopX = false,
            LoopY = false,
            Virtual = false,
            RescueShortcut = "Ctrl+Alt+Shift+M",
            Priority = "High",
            PriorityUnhooked = "Idle",
            // "Strait" is a misspelling kept on purpose, and "Cross" is the other wire
            // value. See AlgorithmWireSpellings below.
            Algorithm = "Cross",
            MaxTravelDistance = 150,
            FreelookCheckInterval = 100,
            FreelookEnabled = true,
        };

        // A name carrying XML metacharacters: monitor names come straight from EDID, and
        // a raw & here breaks the daemon's parser.
        layout.Zones.Add(new Zone(
            leftBorders, "DISPLAY1", "Left & \"Main\"",
            new Rect(-1920, 0, 1920, 1080),
            new Rect(-480, 0, 480, EdgeHeightMm)));
        layout.Zones.Add(new Zone(
            new BorderResistance(), "DISPLAY2", "Right",
            new Rect(0, 0, 1920, 1080),
            new Rect(0, 0, 480, EdgeHeightMm)));

        // Init() computes the links, and reads MaxTravelDistance while doing it: it has
        // to run after the options above.
        layout.Init();
        return layout;
    }

    //================================//
    // UI→daemon: C# is authoritative

    [Fact]
    public void CurrentLayoutSerializationMatchesGolden()
    {
        AssertOwnedGolden("ui-to-daemon/layout-v5.6-current.xml", CurrentLayout().Serialize());
    }

    [Fact]
    public void LayoutGoldenIsWellFormedAndKeepsTheAttributeNamesTheDaemonReads()
    {
        var xml = System.Xml.Linq.XDocument.Parse(ReadGolden("ui-to-daemon/layout-v5.6-current.xml"));
        var root = xml.Root!;
        Assert.Equal("ZonesLayout", root.Name.LocalName);

        // Every attribute the daemon's ZonesLayout::load_from_element reads by name.
        foreach (var name in new[]
                 {
                     "AdjustPointer", "AdjustSpeed", "LoopX", "LoopY", "Virtual",
                     "RescueShortcut", "Priority", "PriorityUnhooked", "Algorithm",
                     "MaxTravelDistance", "FreelookCheckInterval", "FreelookEnabled"
                 })
            Assert.True(root.Attribute(name) is not null, $"ZonesLayout lost attribute {name}");

        var zone = root.Element("MainZones")!.Elements("Zone").First();
        foreach (var name in new[] { "Id", "Name", "DeviceId" })
            Assert.True(zone.Attribute(name) is not null, $"Zone lost attribute {name}");

        // The daemon reads bounds as <name><Rect Left Top Width Height/></name>.
        foreach (var name in new[] { "PixelsBounds", "PhysicalBounds" })
        {
            var rect = zone.Element(name)?.Element("Rect");
            Assert.True(rect is not null, $"Zone lost {name}/Rect");
            foreach (var a in new[] { "Left", "Top", "Width", "Height" })
                Assert.True(rect!.Attribute(a) is not null, $"{name}/Rect lost attribute {a}");
        }

        var link = zone.Element("RightLinks")!.Elements("ZoneLink").First();
        foreach (var name in new[]
                 {
                     "From", "To", "SourceFromPixel", "SourceToPixel", "TargetFromPixel",
                     "TargetToPixel", "BorderResistance", "MoveBlock", "DragResistance",
                     "DragBlock", "TargetId"
                 })
            Assert.True(link.Attribute(name) is not null, $"ZoneLink lost attribute {name}");
    }

    [Fact]
    public void BoolsAndDoublesUseTheSpellingTheDaemonParses()
    {
        // The daemon's XmlHelper port compares booleans against the literal "True" and
        // parses doubles as InvariantCulture. A ToString() that started emitting "true",
        // or a machine with a comma decimal separator, would silently read as false / 0.
        var root = System.Xml.Linq.XDocument.Parse(
            ReadGolden("ui-to-daemon/layout-v5.6-current.xml")).Root!;

        Assert.Equal("True", root.Attribute("AdjustPointer")!.Value);
        Assert.Equal("False", root.Attribute("AdjustSpeed")!.Value);
        Assert.Equal("150", root.Attribute("MaxTravelDistance")!.Value);

        var physical = root.Element("MainZones")!.Elements("Zone").First()
            .Element("PhysicalBounds")!.Element("Rect")!;
        Assert.DoesNotContain(",", physical.Attribute("Width")!.Value);
    }

    [Theory]
    [InlineData(LittleBigMouseCommand.Run, "command-run.xml")]
    [InlineData(LittleBigMouseCommand.Stop, "command-stop.xml")]
    [InlineData(LittleBigMouseCommand.Quit, "command-quit.xml")]
    public void PayloadlessCommandsMatchGoldens(LittleBigMouseCommand command, string golden)
    {
        AssertOwnedGolden("ui-to-daemon/" + golden, new CommandMessage(command).Serialize());
    }

    [Fact]
    public void ShortcutCommandCarriesItsTextPayloadAsAnAttribute()
    {
        // The daemon reads a text Payload either as an attribute or as a child element
        // (protocol::payload_string); the UI emits the attribute form.
        AssertOwnedGolden(
            "ui-to-daemon/command-shortcut.xml",
            CommandMessage.WithText(LittleBigMouseCommand.Shortcut, "Ctrl+Alt+Shift+M").Serialize());
    }

    [Fact]
    public void LoadCommandWrapsTheLayoutInAPayloadElement()
    {
        AssertOwnedGolden(
            "ui-to-daemon/command-load.xml",
            new CommandMessage(LittleBigMouseCommand.Load, CurrentLayout()).Serialize());
    }

    /// <summary>
    /// The wire spellings of the algorithm: the values the UI is allowed to PRODUCE.
    /// <para>
    /// The daemon (<c>zones/layout.rs</c>) understands "Cross" and, as a tolerated alias,
    /// "CornerCrossing"; everything else it reads as "Strait", silently. The alias exists
    /// only because this repository spent a long time documenting and storing
    /// "CornerCrossing" — in <see cref="ILayoutOptions.Algorithm"/>'s doc comment and in
    /// <c>TestData/Persistence/*/layouts/*.json</c> — while the parser did not accept it.
    /// No shipped version ever wrote it (every release's AlgorithmList offers "Strait"
    /// and "Cross"), so it was a documentation-and-fixture divergence, now closed from
    /// both ends.
    /// </para>
    /// <para>
    /// This test guards the producing end: the UI must keep emitting the wire value, not
    /// lean on the alias.
    /// </para>
    /// </summary>
    [Fact]
    public void AlgorithmWireSpellingsAreTheOnesTheDaemonUnderstands()
    {
        string[] understood = ["Strait", "Cross"];

        Assert.Contains(new ILayoutOptions.Design().Algorithm, understood);
        Assert.Contains(new ZonesLayout().Algorithm, understood, StringComparer.OrdinalIgnoreCase);

        var golden = System.Xml.Linq.XDocument.Parse(
            ReadGolden("ui-to-daemon/layout-v5.6-current.xml")).Root!;
        Assert.Contains(golden.Attribute("Algorithm")!.Value, understood);
    }

    //================================//
    // daemon→UI: Rust is authoritative

    /// <summary>
    /// Every event frame the daemon can emit, exactly as it emits it (the file is
    /// regenerated from <c>protocol.rs</c> by the Rust side). The UI must map each to a
    /// known event — an unparsed frame is a daemon state the UI silently never sees.
    /// </summary>
    [Fact]
    public void EveryDaemonEventGoldenParsesToAKnownEvent()
    {
        var lines = ReadGolden("daemon-to-ui/events.txt")
            .Split('\n', StringSplitOptions.RemoveEmptyEntries);

        Assert.NotEmpty(lines);

        var seen = new List<LittleBigMouseEvent>();
        foreach (var line in lines)
        {
            Assert.True(DaemonMessage.TryParse(line, out var message),
                $"the UI cannot parse a frame the daemon emits: {line}");
            seen.Add(message.Event);
        }

        // The states the UI's own logic branches on must all be represented, so that
        // dropping one from the daemon shows up here rather than as a dead branch.
        foreach (var required in new[]
                 {
                     LittleBigMouseEvent.Running, LittleBigMouseEvent.Stopped,
                     LittleBigMouseEvent.Paused, LittleBigMouseEvent.DisplayChanged,
                     LittleBigMouseEvent.Loaded, LittleBigMouseEvent.LoadFailed,
                     LittleBigMouseEvent.Probed, LittleBigMouseEvent.Rescued,
                     LittleBigMouseEvent.ShortcutUnavailable
                 })
            Assert.Contains(required, seen);
    }

    [Fact]
    public void PayloadCarryingEventGoldensKeepTheirPayloadIntact()
    {
        var byEvent = ReadGolden("daemon-to-ui/events.txt")
            .Split('\n', StringSplitOptions.RemoveEmptyEntries)
            .Select(line => DaemonMessage.TryParse(line, out var m) ? m : default)
            .ToLookup(m => m.Event);

        // The daemon XML-escapes payloads; the UI must get the raw text back. A path
        // with & and < in it is the case that broke a naive substring parser.
        var focus = byEvent[LittleBigMouseEvent.FocusChanged].Single();
        Assert.Contains("&", focus.Payload);
        Assert.Contains("<", focus.Payload);
        Assert.DoesNotContain("&amp;", focus.Payload);

        // Loaded's payload is an informative summary, and the UI shows it verbatim.
        Assert.Matches(@"^\d+ zones \(\d+ main\)", byEvent[LittleBigMouseEvent.Loaded].Single().Payload);

        // ShortcutUnavailable names the combination that could not be registered.
        Assert.NotEmpty(byEvent[LittleBigMouseEvent.ShortcutUnavailable].Single().Payload);
    }

    /// <summary>
    /// An event the UI does not know must be REJECTED, not mapped onto something it does
    /// know. Forward compatibility runs this way round: a newer daemon paired with an
    /// older UI degrades to "state unchanged", never to a wrong state.
    /// </summary>
    [Fact]
    public void UnknownEventFromANewerDaemonIsRejected()
    {
        Assert.False(DaemonMessage.TryParse(
            "<DaemonMessage><Event>SomethingNewerEntirely</Event></DaemonMessage>", out _));
    }

    /// <summary>
    /// The probe report the daemon actually emits for the current layout golden — the
    /// file is produced by driving the real Rust engine over that very layout. This is
    /// the only place the two geometric models are compared on the same input: the UI
    /// gets the daemon's own account of which edges cross where.
    /// </summary>
    [Fact]
    public void ProbeReportGoldenParsesAndDescribesTheGoldenLayout()
    {
        Assert.True(ProbeReport.TryParse(ReadGolden("daemon-to-ui/probe-report.xml"), out var report));
        Assert.NotNull(report);

        // The layout golden asks for Cross; the report must say the daemon used it.
        Assert.Equal("Cross", report!.Algorithm);
        Assert.False(report.LoopX);
        Assert.False(report.LoopY);
        Assert.False(report.Virtual);

        // Two monitors in, two probed zones out, keyed by the DeviceIds the UI sent.
        Assert.Equal(2, report.Zones.Count);
        Assert.Equal(["DISPLAY1", "DISPLAY2"], report.Zones.Select(z => z.DeviceId).Order());

        // The name round-tripped through two layers of XML escaping.
        Assert.Contains(report.Zones, z => z.Name == "Left & \"Main\"");

        // Left's right edge crosses into Right (the blocked section blocks drags only,
        // and the prober reports crossability with resistance bypassed).
        var left = report.Zones.Single(z => z.DeviceId == "DISPLAY1");
        var right = left.Edges.Single(e => e.Side == "Right");
        Assert.All(right.Runs, run => Assert.False(run.IsWall));

        // Left's left edge is the outside world.
        var outward = left.Edges.Single(e => e.Side == "Left");
        Assert.All(outward.Runs, run => Assert.True(run.IsWall));
    }
}
