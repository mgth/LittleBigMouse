using LittleBigMouse.Zoning;

namespace LittleBigMouse.DisplayLayout.Tests;

public class DaemonProtocolTests
{
    [Theory]
    [InlineData("Paused", LittleBigMouseEvent.Paused)]
    [InlineData("SettingChanged", LittleBigMouseEvent.SettingsChanged)]
    [InlineData("SettingsChanged", LittleBigMouseEvent.SettingsChanged)]
    [InlineData("Loaded", LittleBigMouseEvent.Loaded)]
    [InlineData("LoadFailed", LittleBigMouseEvent.LoadFailed)]
    public void ParserMapsExactEventElement(string name, LittleBigMouseEvent expected)
    {
        Assert.True(DaemonMessage.TryParse(
            $"<DaemonMessage><Event>{name}</Event></DaemonMessage>", out var message));
        Assert.Equal(expected, message.Event);
    }

    [Fact]
    public void ProbeReport_RoundTripsThroughDaemonMessagePayload()
    {
        // Exact daemon wire shape: the report document is XML-escaped into the payload
        // (protocol::probed in the Rust hook), and comes back out through the standard
        // DaemonMessage parse before ProbeReport.TryParse reads it.
        const string report = "<ProbeReport Algorithm=\"Strait\" LoopX=\"True\" LoopY=\"False\" Virtual=\"True\">"
            + "<Zone Id=\"0\" Name=\"Left &amp; Main\">"
            + "<Edge Side=\"Right\"><Run From=\"0\" To=\"1079\" Target=\"1\"/></Edge>"
            + "<Edge Side=\"Left\"><Run From=\"0\" To=\"624\" Target=\"-1\"/><Run From=\"625\" To=\"1079\" Target=\"2\"/></Edge>"
            + "</Zone></ProbeReport>";
        var wire = "<DaemonMessage><Event>Probed</Event><Payload>"
            + report.Replace("&", "&amp;").Replace("<", "&lt;").Replace(">", "&gt;").Replace("\"", "&quot;")
            + "</Payload></DaemonMessage>";

        Assert.True(DaemonMessage.TryParse(wire, out var message));
        Assert.Equal(LittleBigMouseEvent.Probed, message.Event);

        Assert.True(ProbeReport.TryParse(message.Payload, out var parsed));
        Assert.NotNull(parsed);
        Assert.Equal("Strait", parsed.Algorithm);
        Assert.True(parsed.LoopX);
        Assert.True(parsed.Virtual);

        var zone = Assert.Single(parsed.Zones);
        Assert.Equal("Left & Main", zone.Name);
        Assert.Equal(2, zone.Edges.Count);

        var left = zone.Edges.Single(e => e.Side == "Left");
        Assert.Equal(2, left.Runs.Count);
        Assert.True(left.Runs[0].IsWall);
        Assert.Equal((625, 1079, 2), (left.Runs[1].From, left.Runs[1].To, left.Runs[1].TargetId));
    }

    [Fact]
    public void LoadedEvent_CarriesTheSummaryPayload()
    {
        // Exact daemon wire shape (protocol::loaded in the Rust hook).
        const string xml = "<DaemonMessage><Event>Loaded</Event>" +
                           "<Payload>3 zones (3 main), virtual</Payload></DaemonMessage>";
        Assert.True(DaemonMessage.TryParse(xml, out var message));
        Assert.Equal(LittleBigMouseEvent.Loaded, message.Event);
        Assert.Equal("3 zones (3 main), virtual", message.Payload);
    }

    [Fact]
    public void EventWordsInsideFocusPayloadCannotChangeTheEvent()
    {
        const string xml = "<DaemonMessage><Event>FocusChanged</Event>" +
                           "<Payload>C:\\Games\\Stopped DisplayChanged.exe</Payload></DaemonMessage>";
        Assert.True(DaemonMessage.TryParse(xml, out var message));
        Assert.Equal(LittleBigMouseEvent.FocusChanged, message.Event);
        Assert.Contains("Stopped DisplayChanged.exe", message.Payload);
    }

    [Fact]
    public async Task AtomicWriteKeepsLastGoodBackup()
    {
        var directory = Path.Combine(Path.GetTempPath(), $"lbm-atomic-{Guid.NewGuid():N}");
        var path = Path.Combine(directory, "Current.xml");
        const string first = "<CommandMessage Command=\"Run\"/>\n";
        const string second = "<CommandMessage Command=\"Stop\"/>\n";
        try
        {
            await AtomicRecoveryFile.WriteAsync(path, first, CancellationToken.None);
            await AtomicRecoveryFile.WriteAsync(path, second, CancellationToken.None);
            Assert.Equal(second, await File.ReadAllTextAsync(path));
            Assert.Equal(first, await File.ReadAllTextAsync(path + ".bak"));
            Assert.Empty(Directory.EnumerateFiles(directory, "*.tmp"));
        }
        finally
        {
            if (Directory.Exists(directory)) Directory.Delete(directory, recursive: true);
        }
    }

    [Fact]
    public async Task MarkStoppedKeepsLoadButStripsRun()
    {
        var directory = Path.Combine(Path.GetTempPath(), $"lbm-atomic-{Guid.NewGuid():N}");
        var path = Path.Combine(directory, "Current.xml");
        const string load = "<CommandMessage Command=\"Load\"><Payload></Payload></CommandMessage>";
        const string run = "<CommandMessage Command=\"Run\" Payload=\"\"></CommandMessage>";
        try
        {
            await AtomicRecoveryFile.WriteAsync(path, $"{load}\n{run}\n", CancellationToken.None);
            await AtomicRecoveryFile.MarkStoppedAsync(path, CancellationToken.None);
            Assert.Equal($"{load}\n", await File.ReadAllTextAsync(path));
        }
        finally
        {
            if (Directory.Exists(directory)) Directory.Delete(directory, recursive: true);
        }
    }

    [Fact]
    public async Task MarkStoppedWithoutFileIsANoOp()
    {
        var directory = Path.Combine(Path.GetTempPath(), $"lbm-atomic-{Guid.NewGuid():N}");
        var path = Path.Combine(directory, "Current.xml");
        await AtomicRecoveryFile.MarkStoppedAsync(path, CancellationToken.None);
        Assert.False(File.Exists(path));
    }

    [Fact]
    public void SerializedAttributesEscapeXmlMetacharacters()
    {
        var serialized = ZoneSerializer.Serialize(
            new NamedThing { Name = "L&G \"32<40>\" TV" }, thing => thing.Name);

        using var reader = System.Xml.XmlReader.Create(new StringReader(serialized));
        Assert.True(reader.Read());
        Assert.Equal("L&G \"32<40>\" TV", reader.GetAttribute("Name"));
    }

    class NamedThing
    {
        public string Name { get; set; } = "";
    }

    [Fact]
    public async Task InvalidRecoveryXmlDoesNotReplaceCurrentFile()
    {
        var directory = Path.Combine(Path.GetTempPath(), $"lbm-atomic-{Guid.NewGuid():N}");
        var path = Path.Combine(directory, "Current.xml");
        const string good = "<CommandMessage Command=\"Run\"/>\n";
        try
        {
            await AtomicRecoveryFile.WriteAsync(path, good, CancellationToken.None);
            await Assert.ThrowsAsync<System.Xml.XmlException>(() =>
                AtomicRecoveryFile.WriteAsync(path, "<truncated", CancellationToken.None));
            Assert.Equal(good, await File.ReadAllTextAsync(path));
        }
        finally
        {
            if (Directory.Exists(directory)) Directory.Delete(directory, recursive: true);
        }
    }
}
