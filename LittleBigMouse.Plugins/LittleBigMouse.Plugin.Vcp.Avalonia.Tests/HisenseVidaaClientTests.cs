using LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;
using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

/// <summary>
/// The client as its callers see it — pairing, commands and answers — over
/// <see cref="FakeVidaaTransport"/> rather than a device on the network.
/// </summary>
public class HisenseVidaaClientTests
{
    const string ControllerMac = "9c:69:b4:61:a9:78";
    const string DeviceMac = "56:b8:88:4e:f7:19";
    const string DeviceClientId = "56:b8:88:4e:f7:19$his$256DBF_vidaacommon_001";
    const string RemoteNowTopic = "9C:69:B4:61:A9:78$normal";

    static HisenseVidaaConfiguration Paired() => new()
    {
        MonitorId = "HEC002F",
        IpAddress = "192.168.0.181",
        ProtocolVersion = 3290,
        ClientId = DeviceClientId,
        MqttUsername = "his$6239759786168176024",
        AccessToken = "access-token",
    };

    /// <summary>A device known but never paired, with the controller MAC already resolved.</summary>
    static HisenseVidaaConfiguration Unpaired(int? protocolVersion) => new()
    {
        MonitorId = "HEC002F",
        IpAddress = "192.168.0.181",
        ProtocolVersion = protocolVersion,
        ControllerMacAddress = ControllerMac,
        DeviceUuid = DeviceMac,
    };

    [Fact]
    public async Task SendsTheTranslatedKeyOnThePairedSession()
    {
        var transport = new FakeVidaaTransport();
        await using var client = new HisenseVidaaClient(Paired(), transport.OpenAsync);

        await client.SendKeyAsync("KEY_VOLUP", default);

        Assert.True(client.Connected);
        Assert.Equal(
            ($"/remoteapp/tv/remote_service/{DeviceClientId}/actions/sendkey", "KEY_VOLUMEUP"),
            Assert.Single(transport.Current.Published));
    }

    [Fact]
    public async Task ReconnectsAndSendsAgainWhenTheDeviceDroppedTheSession()
    {
        var transport = new FakeVidaaTransport();
        await using var client = new HisenseVidaaClient(Paired(), transport.OpenAsync);
        await client.SendKeyAsync("KEY_OK", default);
        transport.Current.PublishError = new IOException("write found the connection closed");

        await client.SendKeyAsync("KEY_OK", default);

        Assert.Equal(2, transport.Connections.Count);
        Assert.True(transport.Connections[0].Disposed);
        Assert.Equal("KEY_OK", Assert.Single(transport.Current.Published).Payload);
    }

    [Fact]
    public async Task ReadsTheVolumeTheDeviceBroadcastsBack()
    {
        var transport = new FakeVidaaTransport
        {
            OnOpened = connection => connection.Answer = (device, topic, _) =>
            {
                if (topic.EndsWith("/actions/getvolume", StringComparison.Ordinal))
                    device.Receive(
                        "/remoteapp/mobile/broadcast/platform_service/actions/volumechange",
                        "{\"volume_type\":0,\"volume_value\":37}");
            },
        };
        await using var client = new HisenseVidaaClient(Paired(), transport.OpenAsync);

        Assert.Equal(37, await client.GetVolumeAsync(default));
        Assert.Equal(
            ($"/remoteapp/tv/platform_service/{DeviceClientId}/actions/getvolume", "0"),
            Assert.Single(transport.Current.Published));
    }

    [Fact]
    public async Task ReadsThePictureSettingsTheDeviceLists()
    {
        var transport = new FakeVidaaTransport
        {
            OnOpened = connection => connection.Answer = (device, _, payload) =>
            {
                if (payload == HisenseVidaaProtocol.PictureSettingsRequestPayload())
                    device.Receive(
                        "/remoteapp/mobile/broadcast/platform_service/data/picturesetting",
                        "{\"menu_info\":[{\"menu_id\":2,\"menu_name\":\"Laser Luminance\",\"menu_value\":5,\"menu_value_type\":\"int\",\"menu_flag\":1}]}");
            },
        };
        await using var client = new HisenseVidaaClient(Paired(), transport.OpenAsync);

        var settings = await client.GetPictureSettingsAsync(default);

        Assert.Equal(new VidaaPictureSetting(2, "Laser Luminance", "5", "int", 1), Assert.Single(settings));
    }

    [Fact]
    public async Task RefusesCommandsOnceTheClientIsDisposed()
    {
        var transport = new FakeVidaaTransport();
        var client = new HisenseVidaaClient(Paired(), transport.OpenAsync);
        await client.SendKeyAsync("KEY_OK", default);

        await client.DisposeAsync();

        Assert.True(transport.Current.Disposed);
        Assert.False(client.Connected);
        await Assert.ThrowsAsync<ObjectDisposedException>(() => client.SendKeyAsync("KEY_OK", default));
    }

    [Fact]
    public async Task PairsARemoteNowDeviceOnItsOwnTopic()
    {
        var configuration = Unpaired(2160);
        var transport = new FakeVidaaTransport();
        await using var client = new HisenseVidaaClient(configuration, transport.OpenAsync);

        await client.StartPairingAsync(default);

        Assert.Equal(VidaaAuthMethod.Legacy, configuration.AuthMethod);
        Assert.Equal(RemoteNowTopic, configuration.ClientId);
        Assert.False(configuration.LegacyAuthorized);
        Assert.Equal(
            HisenseVidaaProtocol.LegacyResponseTopics(RemoteNowTopic),
            transport.Current.Subscribed);
        Assert.Equal(
            ($"/remoteapp/tv/ui_service/{RemoteNowTopic}/actions/gettvstate", ""),
            Assert.Single(transport.Current.Published));
    }

    [Fact]
    public async Task AuthorisesARemoteNowDeviceWithTheDisplayedPin()
    {
        var configuration = Unpaired(2160);
        var transport = new FakeVidaaTransport();
        await using var client = new HisenseVidaaClient(configuration, transport.OpenAsync);
        await client.StartPairingAsync(default, requestPin: false);
        transport.Current.Answer = (device, topic, _) =>
        {
            if (topic.EndsWith("/actions/authenticationcode", StringComparison.Ordinal))
                device.Receive($"/remoteapp/mobile/{RemoteNowTopic}/ui_service/data/authentication", "");
        };

        await client.AuthenticateAsync("1234", default);

        Assert.True(configuration.LegacyAuthorized);
        Assert.True(configuration.HasPairing);
        Assert.Equal(
            ($"/remoteapp/tv/ui_service/{RemoteNowTopic}/actions/authenticationcode", "{\"authNum\":\"1234\"}"),
            Assert.Single(transport.Current.Published));
    }

    [Fact]
    public async Task TriesTheKnownCredentialVariantsUntilTheDeviceLetsOneIn()
    {
        var configuration = Unpaired(protocolVersion: null);
        var transport = new FakeVidaaTransport();
        transport.Refuse(new UnauthorizedAccessException("VIDAA rejected the generated username or password."));
        // A broker that goes silent after a rejection must not stop the sweep either.
        transport.Refuse(new OperationCanceledException());
        await using var client = new HisenseVidaaClient(configuration, transport.OpenAsync);

        await client.StartPairingAsync(default);

        Assert.Equal(3, transport.Requests.Count);
        Assert.Equal(DeviceClientId, configuration.ClientId);
        Assert.Equal(DeviceMac, configuration.DeviceUuid);
        Assert.Equal(VidaaAuthMethod.Middle, configuration.AuthMethod);
        Assert.Equal(transport.Requests[2].Username, configuration.MqttUsername);
        Assert.Equal(
            ($"/remoteapp/tv/ui_service/{DeviceClientId}/actions/vidaa_app_connect",
                HisenseVidaaProtocol.PairingRequestPayload()),
            Assert.Single(transport.Current.Published));
    }

    [Fact]
    public async Task ReportsThatTheDeviceRejectedEveryCredentialVariant()
    {
        var configuration = Unpaired(protocolVersion: null);
        var transport = new FakeVidaaTransport();
        var last = new UnauthorizedAccessException("VIDAA refused authorization.");
        for (var attempt = 0; attempt < 5; attempt++)
            transport.Refuse(new UnauthorizedAccessException("VIDAA rejected the generated username or password."));
        transport.Refuse(last);
        await using var client = new HisenseVidaaClient(configuration, transport.OpenAsync);

        var error = await Assert.ThrowsAsync<UnauthorizedAccessException>(
            () => client.StartPairingAsync(default));

        // Three authentication methods over the two spellings of the device identity.
        Assert.Equal(6, transport.Requests.Count);
        Assert.Contains("protocol unknown", error.Message);
        Assert.Contains(DeviceMac, error.Message);
        Assert.Same(last, error.InnerException);
    }

    [Fact]
    public async Task TradesThePinForTheTokensLaterSessionsReuse()
    {
        var configuration = Unpaired(3290);
        var transport = new FakeVidaaTransport();
        await using var client = new HisenseVidaaClient(configuration, transport.OpenAsync);
        await client.StartPairingAsync(default);
        var clientId = configuration.ClientId;
        transport.Current.Answer = (device, topic, _) =>
        {
            if (topic.EndsWith("/actions/authenticationcode", StringComparison.Ordinal))
                device.Receive($"/remoteapp/mobile/{clientId}/ui_service/data/authentication", "{\"result\":1}");
            if (topic == HisenseVidaaProtocol.TokenRequestTopic(clientId))
                device.Receive(
                    $"/remoteapp/mobile/{clientId}/platform_service/data/tokenissuance",
                    "{\"accesstoken\":\"access\",\"refreshtoken\":\"refresh\",\"accesstoken_duration_day\":5}");
        };

        await client.AuthenticateAsync("1234", default);

        Assert.Equal("access", configuration.AccessToken);
        Assert.Equal("refresh", configuration.RefreshToken);
        Assert.True(configuration.HasPairing);
        Assert.Equal(
            [
                $"/remoteapp/tv/ui_service/{clientId}/actions/vidaa_app_connect",
                $"/remoteapp/tv/ui_service/{clientId}/actions/authenticationcode",
                HisenseVidaaProtocol.TokenRequestTopic(clientId),
                $"/remoteapp/tv/ui_service/{clientId}/actions/authenticationcodeclose",
            ],
            transport.Current.Published.Select(message => message.Topic));
    }

    [Fact]
    public async Task ReportsAPinTheDeviceRejected()
    {
        var configuration = Unpaired(3290);
        var transport = new FakeVidaaTransport();
        await using var client = new HisenseVidaaClient(configuration, transport.OpenAsync);
        await client.StartPairingAsync(default);
        var clientId = configuration.ClientId;
        transport.Current.Answer = (device, topic, _) =>
        {
            if (topic.EndsWith("/actions/authenticationcode", StringComparison.Ordinal))
                device.Receive($"/remoteapp/mobile/{clientId}/ui_service/data/authentication", "{\"result\":0}");
        };

        var error = await Assert.ThrowsAsync<UnauthorizedAccessException>(
            () => client.AuthenticateAsync("1234", default));

        Assert.Equal("The VIDAA PIN was rejected.", error.Message);
        Assert.False(configuration.HasPairing);
    }

    [Fact]
    public async Task RefusesToAuthenticateWithoutAnOpenSession()
    {
        var transport = new FakeVidaaTransport();
        await using var client = new HisenseVidaaClient(Unpaired(3290), transport.OpenAsync);

        var error = await Assert.ThrowsAsync<InvalidOperationException>(
            () => client.AuthenticateAsync("1234", default));

        Assert.Equal("The VIDAA connection is not open.", error.Message);
        Assert.Empty(transport.Requests);
    }
}
