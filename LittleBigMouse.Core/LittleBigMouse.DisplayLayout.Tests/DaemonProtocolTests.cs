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
