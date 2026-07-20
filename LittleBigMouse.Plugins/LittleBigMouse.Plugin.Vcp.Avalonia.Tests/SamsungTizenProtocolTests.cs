using System.Text.Json;
using Avalonia.Input;
using LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;
using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

public class SamsungTizenProtocolTests
{
    [Fact]
    public void RemoteUriUsesSecureEndpointAndToken()
    {
        var uri = SamsungTizenProtocol.RemoteUri("192.168.1.42", "12 34");

        Assert.Equal("wss", uri.Scheme);
        Assert.Equal(8002, uri.Port);
        Assert.Contains("samsung.remote.control", uri.AbsolutePath);
        Assert.Contains("token=12%2034", uri.Query);
    }

    [Fact]
    public void RemotePayloadIsAKeyClick()
    {
        using var json = JsonDocument.Parse(SamsungTizenProtocol.RemoteKeyPayload("KEY_HOME"));
        var parameters = json.RootElement.GetProperty("params");

        Assert.Equal("ms.remote.control", json.RootElement.GetProperty("method").GetString());
        Assert.Equal("Click", parameters.GetProperty("Cmd").GetString());
        Assert.Equal("KEY_HOME", parameters.GetProperty("DataOfCmd").GetString());
        Assert.Equal("SendRemoteKey", parameters.GetProperty("TypeOfRemote").GetString());
    }

    [Fact]
    public void ParsesG80DeviceDescription()
    {
        const string payload = """
            {
              "device": {
                "name": "Odyssey OLED G8",
                "modelName": "S32DG80",
                "duid": "uuid:abc",
                "wifiMac": "AA:BB:CC:DD:EE:FF"
              }
            }
            """;

        var device = SamsungTizenProtocol.ParseDevice("192.168.1.42", payload);

        Assert.Equal("Odyssey OLED G8", device.Name);
        Assert.Equal("S32DG80", device.ModelName);
        Assert.Equal("uuid:abc", device.DeviceId);
        Assert.Equal("AA:BB:CC:DD:EE:FF", device.MacAddress);
    }

    [Fact]
    public void ParsesNestedPairingToken()
    {
        const string payload = """
            {
              "event": "ms.channel.connect",
              "data": {
                "clients": [
                  { "attributes": { "name": "LBM", "token": "19067219" } }
                ]
              }
            }
            """;

        var result = SamsungTizenProtocol.ParseChannelEvent(payload);

        Assert.True(result.Connected);
        Assert.Equal("19067219", result.Token);
        Assert.Empty(result.Error);
    }

    [Fact]
    public void ParsesPairingTimeoutAsAnError()
    {
        var result = SamsungTizenProtocol.ParseChannelEvent("""{"event":"ms.channel.timeOut"}""");

        Assert.False(result.Connected);
        Assert.Equal("ms.channel.timeOut", result.Error);
    }

    [Theory]
    [InlineData(Key.Up, "KEY_UP")]
    [InlineData(Key.Enter, "KEY_ENTER")]
    [InlineData(Key.Escape, "KEY_RETURN")]
    [InlineData(Key.VolumeUp, "KEY_VOLUP")]
    [InlineData(Key.PageDown, "KEY_CHDOWN")]
    [InlineData(Key.MediaPlayPause, "KEY_PLAYPAUSE")]
    [InlineData(Key.MediaInfo, "KEY_INFO")]
    [InlineData(Key.NumPad7, "KEY_7")]
    public void MapsKeyboardInputToRemoteKeys(Key key, string expected)
        => Assert.Equal(expected, SamsungRemoteKeyboard.RemoteKeyFor(key));

    [Fact]
    public void LeavesUnrelatedKeyboardInputUnhandled()
        => Assert.Null(SamsungRemoteKeyboard.RemoteKeyFor(Key.Tab));

    [Fact]
    public void ParsesRemoteMacroAndAppliesExplicitDelay()
    {
        var sequence = SamsungTizenProtocol.ParseSequence("KEY_MENU,700+KEY_DOWN;KEY_ENTER");

        Assert.Equal(3, sequence.Count);
        Assert.Equal("KEY_MENU", sequence[0].Key);
        Assert.Equal(TimeSpan.FromMilliseconds(700), sequence[0].DelayAfter);
        Assert.Equal(TimeSpan.FromMilliseconds(150), sequence[1].DelayAfter);
    }

    [Fact]
    public void CreatesStandardWakeOnLanPacket()
    {
        var packet = WakeOnLan.CreateMagicPacket("AA:BB:CC:DD:EE:FF");

        Assert.Equal(102, packet.Length);
        Assert.All(packet.Take(6), value => Assert.Equal(0xff, value));
        for (var offset = 6; offset < packet.Length; offset += 6)
            Assert.Equal(new byte[] { 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff }, packet.Skip(offset).Take(6));
    }

    [Fact]
    public void ParsesSsdpHeadersCaseInsensitively()
    {
        const string response = "HTTP/1.1 200 OK\r\nLocation: http://192.168.1.42:9197/dmr\r\n\r\n";
        Assert.Equal(
            "http://192.168.1.42:9197/dmr",
            SamsungTizenDiscovery.ParseHeader(response, "LOCATION"));
    }

    [Fact]
    public void SettingsRoundTripWithoutSharingMutableInstances()
    {
        var directory = Path.Combine(Path.GetTempPath(), "lbm-tizen-tests", Guid.NewGuid().ToString("N"));
        var path = Path.Combine(directory, "settings.json");
        try
        {
            var store = new SamsungTizenSettingsStore(path);
            var configuration = new SamsungTizenConfiguration
            {
                MonitorId = "SAM123",
                IpAddress = "192.168.1.42",
                Token = "secret",
            };

            store.Save(configuration);
            configuration.Token = "changed";

            var loaded = new SamsungTizenSettingsStore(path).Get("SAM123");
            Assert.NotNull(loaded);
            Assert.Equal("secret", loaded.Token);
            Assert.DoesNotContain("secret", File.ReadAllText(path), StringComparison.Ordinal);
        }
        finally
        {
            if (Directory.Exists(directory)) Directory.Delete(directory, recursive: true);
        }
    }
}
