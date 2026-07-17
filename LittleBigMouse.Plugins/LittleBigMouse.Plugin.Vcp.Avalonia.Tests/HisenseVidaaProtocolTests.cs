using LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;
using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

public class HisenseVidaaProtocolTests
{
    [Theory]
    [InlineData(2160, VidaaAuthMethod.Legacy)]
    [InlineData(3000, VidaaAuthMethod.Middle)]
    [InlineData(3285, VidaaAuthMethod.Middle)]
    [InlineData(3290, VidaaAuthMethod.Modern)]
    public void SelectsAuthenticationFromTransportProtocol(int version, VidaaAuthMethod expected)
        => Assert.Equal(expected, HisenseVidaaProtocol.AuthMethodFor(version));

    [Fact]
    public void C1UsesStaticRemoteNowIdentity()
    {
        Assert.True(HisenseVidaaProtocol.UsesStaticLegacyProtocol(2160));
        Assert.Equal("9C:69:B4:61:A9:78$normal",
            HisenseVidaaProtocol.LegacyDeviceTopic("9c:69:b4:61:a9:78"));
        Assert.Contains("\"authNum\":\"1234\"", HisenseVidaaProtocol.LegacyPinPayload("1234"));
    }

    [Fact]
    public void GeneratesKnownModernCredentials()
    {
        var credentials = HisenseVidaaProtocol.GenerateCredentials(
            "56:b8:88:4e:f7:19", VidaaAuthMethod.Modern, 1766974704);

        Assert.Equal("56:b8:88:4e:f7:19$his$256DBF_vidaacommon_001", credentials.ClientId);
        Assert.Equal("his$6239759786168176024", credentials.Username);
        Assert.Equal("C3BA44782E18ABF4892AC44D79A622D2", credentials.Password);
    }

    [Fact]
    public void ParsesC1LegacyDescriptor()
    {
        const string xml = """
            <root><device><friendlyName>projo</friendlyName><modelName>Renderer</modelName>
            <modelDescription>mac=903c1d26e39c
            macWifi=bc5c17272aa0
            transport_protocol=2160
            vidaa_support=1</modelDescription></device></root>
            """;

        var device = HisenseVidaaProtocol.ParseDescriptor("192.168.0.181", xml);

        Assert.Equal(2160, device.ProtocolVersion);
        Assert.Equal("90:3C:1D:26:E3:9C", device.MacAddress);
    }

    [Theory]
    [InlineData("KEY_VOLUP", "KEY_VOLUMEUP")]
    [InlineData("KEY_VOLDOWN", "KEY_VOLUMEDOWN")]
    [InlineData("KEY_PLAYPAUSE", "KEY_PLAY")]
    [InlineData("KEY_FF", "KEY_FORWARDS")]
    [InlineData("KEY_REWIND", "KEY_BACKWARDS")]
    [InlineData("KEY_MUTE", "KEY_MUTE")]
    [InlineData("KEY_STOP", "KEY_STOP")]
    public void TranslatesSharedRemoteKeysToVidaaCodes(string key, string expected)
        => Assert.Equal(expected, HisenseVidaaProtocol.TranslateKey(key));

    [Fact]
    public void ParsesExperimentalVidaaKeyMacro()
    {
        var sequence = RemoteMacro.Parse("KEY_BRIGHTNESSUP,700,KEY_BRIGHTNESSDOWN");

        Assert.Equal(2, sequence.Count);
        Assert.Equal("KEY_BRIGHTNESSUP", sequence[0].Key);
        Assert.Equal(TimeSpan.FromMilliseconds(700), sequence[0].DelayAfter);
        Assert.Equal("KEY_BRIGHTNESSDOWN", sequence[1].Key);
    }

    [Theory]
    [InlineData("/remoteapp/mobile/client/ui_service/data/tokenissuance", "{}")]
    [InlineData("/some/topic", "{\"accesstoken\":\"secret\"}")]
    [InlineData("/remoteapp/tv/ui_service/client/actions/authenticationcode", "1234")]
    public void FiltersSensitiveMqttCaptureMessages(string topic, string payload)
        => Assert.True(VidaaTrafficCapture.IsSensitive(topic, payload));

    [Fact]
    public void KeepsPictureSettingEventsInMqttCapture()
        => Assert.False(VidaaTrafficCapture.IsSensitive(
            "/remoteapp/mobile/broadcast/platform_service/data/picturesetting",
            "{\"menu_info\":[{\"menu_id\":14,\"menu_flag\":6}]}"));

    [Fact]
    public void GeneratesPictureSettingCommand()
        => Assert.Equal(
            "{\"action\":\"set_value\",\"menu_id\":2,\"menu_value\":5,\"menu_value_type\":\"int\"}",
            HisenseVidaaProtocol.PictureSettingPayload(2, 5));

    [Fact]
    public void GeneratesPictureSettingsCatalogueRequest()
        => Assert.Equal("{\"action\":\"get_menu_info\"}",
            HisenseVidaaProtocol.PictureSettingsRequestPayload());

    [Fact]
    public void ParsesPictureSettingsCatalogue()
    {
        var parsed = HisenseVidaaProtocol.TryParsePictureSettings(
            "/remoteapp/mobile/broadcast/platform_service/data/picturesetting",
            """
            {"action":"notify_value_changed","menu_info":[
              {"menu_flag":1,"menu_id":2,"menu_name":"Laser Luminance","menu_value":5,"menu_value_type":"int"},
              {"menu_flag":1,"menu_id":7,"menu_name":"Brightness","menu_value":50,"menu_value_type":"int"}
            ]}
            """,
            out var settings);

        Assert.True(parsed);
        Assert.Collection(settings,
            laser => Assert.Equal(new VidaaPictureSetting(2, "Laser Luminance", "5", "int", 1), laser),
            brightness => Assert.Equal(new VidaaPictureSetting(7, "Brightness", "50", "int", 1), brightness));
    }

    [Fact]
    public void ParsesSinglePictureSettingNotification()
    {
        var parsed = HisenseVidaaProtocol.TryParsePictureSettings(
            "/remoteapp/mobile/broadcast/platform_service/data/picturesetting",
            "{\"action\":\"notify_value_changed\",\"menu_id\":19,\"menu_value\":3,\"menu_value_type\":\"int\"}",
            out var settings);

        Assert.True(parsed);
        Assert.Equal(new VidaaPictureSetting(19, "", "3", "int", null), Assert.Single(settings));
    }

    [Fact]
    public void GeneratesAbsoluteVolumeCommand()
        => Assert.Equal("37", HisenseVidaaProtocol.VolumePayload(37));

    [Fact]
    public void GeneratesExperimentalPlatformLevelCommand()
    {
        Assert.Equal("changelaserluminance",
            HisenseVidaaProtocol.PlatformActionName(" changelaserluminance "));
        Assert.Equal("5", HisenseVidaaProtocol.ExperimentalLevelPayload(5));
    }

    [Theory]
    [InlineData("")]
    [InlineData("platform_service/changelaserluminance")]
    [InlineData("change laser luminance")]
    public void RejectsUnsafeExperimentalPlatformAction(string action)
        => Assert.Throws<ArgumentException>(() => HisenseVidaaProtocol.PlatformActionName(action));

    [Fact]
    public void ParsesVolumeBroadcast()
    {
        var parsed = HisenseVidaaProtocol.TryParseVolume(
            "/remoteapp/mobile/broadcast/platform_service/actions/volumechange",
            "{\"volume_type\":0,\"volume_value\":37}",
            out var volume);

        Assert.True(parsed);
        Assert.Equal(37, volume);
    }
}
