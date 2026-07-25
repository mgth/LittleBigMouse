using LittleBigMouse.Zoning;

namespace LittleBigMouse.DisplayLayout.Tests;

public class DaemonProtocolTests
{
    [Theory]
    [InlineData("Paused", LittleBigMouseEvent.Paused)]
    [InlineData("SettingChanged", LittleBigMouseEvent.SettingsChanged)]
    [InlineData("SettingsChanged", LittleBigMouseEvent.SettingsChanged)]
    public void ParserMapsExactEventElement(string name, LittleBigMouseEvent expected)
    {
        Assert.True(DaemonMessage.TryParse(
            $"<DaemonMessage><Event>{name}</Event></DaemonMessage>", out var message));
        Assert.Equal(expected, message.Event);
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
