using LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;
using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

/// <summary>
/// The connection a VIDAA device accepts, driven over <see cref="FakeVidaaTransport"/>: what
/// opens it, what reopens it, and what it refuses to do without a pairing.
/// </summary>
public class VidaaSessionTests
{
    static HisenseVidaaConfiguration Paired() => new()
    {
        MonitorId = "HEC002F",
        IpAddress = "192.168.0.181",
        ProtocolVersion = 3290,
        ClientId = "56:b8:88:4e:f7:19$his$256DBF_vidaacommon_001",
        MqttUsername = "his$6239759786168176024",
        AccessToken = "access-token",
        ClientCertificatePath = "/home/user/.config/LittleBigMouse/vidaa-client.p12",
        ClientCertificatePassword = "certificate-password",
    };

    static HisenseVidaaConfiguration PairedRemoteNow() => new()
    {
        MonitorId = "HEC002F",
        IpAddress = "192.168.0.181",
        ProtocolVersion = 2160,
        ClientId = "9C:69:B4:61:A9:78$normal",
        MqttUsername = HisenseVidaaProtocol.LegacyMqttUsername,
        LegacyAuthorized = true,
        ClientCertificatePath = "/home/user/.config/LittleBigMouse/vidaa-client.p12",
        ClientCertificatePassword = "certificate-password",
    };

    [Fact]
    public async Task OpensThePairedSessionAndSubscribesTheAnswerTopics()
    {
        var configuration = Paired();
        var transport = new FakeVidaaTransport();
        var session = new VidaaSession(configuration, transport.OpenAsync);

        await session.SendAsync("/remoteapp/tv/remote_service/client/actions/sendkey", "KEY_OK", default);

        var request = Assert.Single(transport.Requests);
        Assert.Equal(configuration.IpAddress, request.Host);
        Assert.Equal(configuration.ClientId, request.ClientId);
        Assert.Equal(configuration.MqttUsername, request.Username);
        Assert.Equal(configuration.AccessToken, request.Password);
        Assert.Equal(configuration.ClientCertificatePath, request.CertificatePath);
        Assert.Equal(configuration.ClientCertificatePassword, request.CertificatePassword);
        Assert.Equal(HisenseVidaaProtocol.ResponseTopics(configuration.ClientId), transport.Current.Subscribed);
        Assert.Equal(
            ("/remoteapp/tv/remote_service/client/actions/sendkey", "KEY_OK"),
            Assert.Single(transport.Current.Published));
    }

    [Fact]
    public async Task KeepsOneSessionForConsecutiveCommands()
    {
        var transport = new FakeVidaaTransport();
        var session = new VidaaSession(Paired(), transport.OpenAsync);

        await session.SendAsync("first", "1", default);
        await session.SendAsync("second", "2", default);

        Assert.Single(transport.Connections);
        Assert.Equal(2, transport.Current.Published.Count);
    }

    [Fact]
    public async Task RefusesToOpenASessionTheDeviceHasNotPaired()
    {
        var configuration = Paired();
        configuration.AccessToken = "";
        var transport = new FakeVidaaTransport();
        var session = new VidaaSession(configuration, transport.OpenAsync);

        var error = await Assert.ThrowsAsync<InvalidOperationException>(
            () => session.SendAsync("topic", "payload", default));

        Assert.Equal("Pair this Hisense VIDAA device first.", error.Message);
        Assert.Empty(transport.Requests);
    }

    [Fact]
    public async Task ClosesTheDeadSessionThenSendsAgainWhenAWriteFindsItGone()
    {
        var transport = new FakeVidaaTransport();
        // The broker accepts one persistent session at a time, so the dead one has to be closed
        // before another is asked for.
        transport.OnOpened = _ => Assert.All(transport.Connections, previous => Assert.True(previous.Disposed));
        var session = new VidaaSession(Paired(), transport.OpenAsync);
        await session.SendAsync("first", "1", default);
        transport.Current.PublishError = new IOException("write found the connection closed");

        await session.SendAsync("sendkey", "KEY_OK", default);

        Assert.Equal(2, transport.Connections.Count);
        Assert.True(transport.Connections[0].Disposed);
        Assert.Equal(("sendkey", "KEY_OK"), Assert.Single(transport.Current.Published));
    }

    [Fact]
    public async Task DoesNotReopenASessionForACommandTheDeviceRefused()
    {
        var transport = new FakeVidaaTransport();
        var session = new VidaaSession(Paired(), transport.OpenAsync);
        await session.SendAsync("first", "1", default);
        transport.Current.PublishError = new UnauthorizedAccessException("the device denied the command");

        await Assert.ThrowsAsync<UnauthorizedAccessException>(
            () => session.SendAsync("sendkey", "KEY_OK", default));

        Assert.Single(transport.Connections);
    }

    [Fact]
    public async Task ReportsTheSecondFailureWhenTheReopenedSessionDiesToo()
    {
        var transport = new FakeVidaaTransport
        {
            OnOpened = connection => connection.PublishError = new IOException("write 2 found the connection closed"),
        };
        var session = new VidaaSession(Paired(), transport.OpenAsync);
        await session.EnsureOpenAsync(default);

        var error = await Assert.ThrowsAsync<IOException>(
            () => session.SendAsync("sendkey", "KEY_OK", default));

        Assert.Equal("write 2 found the connection closed", error.Message);
        Assert.Equal(2, transport.Connections.Count);
    }

    [Fact]
    public async Task ForwardsWhatTheDevicePublishesUntilTheSessionIsClosed()
    {
        var transport = new FakeVidaaTransport();
        var session = new VidaaSession(Paired(), transport.OpenAsync);
        var topics = new List<string>();
        session.MessageReceived += (topic, _, _) => topics.Add(topic);
        await session.EnsureOpenAsync(default);
        var connection = transport.Current;

        connection.Receive("/remoteapp/mobile/broadcast/ui_service/state", "{}");
        await session.CloseAsync();
        connection.Receive("/remoteapp/mobile/broadcast/ui_service/state", "{}");

        Assert.Equal(["/remoteapp/mobile/broadcast/ui_service/state"], topics);
        Assert.True(connection.Disposed);
        Assert.False(session.Connected);
    }

    [Fact]
    public async Task RemoteNowSessionsUseTheServiceAccountAndTheirOwnAnswerTopics()
    {
        var configuration = PairedRemoteNow();
        var transport = new FakeVidaaTransport();
        var session = new VidaaSession(configuration, transport.OpenAsync);

        await session.EnsureOpenAsync(default);
        var first = transport.Requests[0].ClientId;
        await session.CloseAsync();
        await session.EnsureOpenAsync(default);

        Assert.All(transport.Requests, request =>
        {
            Assert.Equal(HisenseVidaaProtocol.LegacyMqttUsername, request.Username);
            Assert.Equal(HisenseVidaaProtocol.LegacyMqttPassword, request.Password);
            Assert.StartsWith("LittleBigMouse/", request.ClientId);
        });
        // The service account is shared, so each session takes an identifier of its own.
        Assert.NotEqual(first, transport.Requests[1].ClientId);
        Assert.Equal(
            HisenseVidaaProtocol.LegacyResponseTopics(configuration.ClientId),
            transport.Current.Subscribed);
    }

    [Fact]
    public async Task DoesNotOpenASessionForACancelledCommand()
    {
        var transport = new FakeVidaaTransport();
        var session = new VidaaSession(Paired(), transport.OpenAsync);
        using var cancellation = new CancellationTokenSource();
        await cancellation.CancelAsync();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(
            () => session.SendAsync("sendkey", "KEY_OK", cancellation.Token));

        Assert.Empty(transport.Requests);
        Assert.False(session.Connected);
    }

    [Fact]
    public async Task RefusesToPublishOnASessionThatIsNotOpen()
    {
        var transport = new FakeVidaaTransport();
        var session = new VidaaSession(Paired(), transport.OpenAsync);

        var error = await Assert.ThrowsAsync<InvalidOperationException>(
            () => session.PublishAsync("topic", "payload", default));

        Assert.Equal("The VIDAA connection is not open.", error.Message);
    }
}
