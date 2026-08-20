using System.Text;
using LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;
using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

/// <summary>
/// What the device publishes, read as the answer a command is waiting for. The formats themselves
/// are covered by <see cref="HisenseVidaaProtocolTests"/>; here it is the routing that matters.
/// </summary>
public class VidaaResponseRouterTests
{
    const string VolumeTopic = "/remoteapp/mobile/broadcast/platform_service/actions/volumechange";
    const string PictureSettingTopic = "/remoteapp/mobile/broadcast/platform_service/data/picturesetting";
    const string AuthenticationTopic = "/remoteapp/mobile/client/ui_service/data/authentication";
    const string TokenTopic = "/remoteapp/mobile/client/ui_service/data/tokenissuance";

    static void Deliver(VidaaResponseRouter router, string topic, string payload)
        => router.Handle(topic, Encoding.UTF8.GetBytes(payload), retained: false);

    [Fact]
    public async Task AnswersTheVolumeCommandWaitingForIt()
    {
        var router = new VidaaResponseRouter(new HisenseVidaaConfiguration());

        var volume = await router.Volume.CollectAsync(
            _ =>
            {
                Deliver(router, VolumeTopic, "{\"volume_type\":0,\"volume_value\":37}");
                return Task.CompletedTask;
            },
            TimeSpan.FromSeconds(1), default);

        Assert.Equal(37, volume);
    }

    [Fact]
    public async Task AnswersThePictureSettingsCommandWaitingForIt()
    {
        var router = new VidaaResponseRouter(new HisenseVidaaConfiguration());

        var settings = await router.PictureSettings.CollectAsync(
            _ =>
            {
                Deliver(router, PictureSettingTopic,
                    "{\"menu_info\":[{\"menu_id\":2,\"menu_name\":\"Laser Luminance\",\"menu_value\":5,\"menu_value_type\":\"int\",\"menu_flag\":1}]}");
                return Task.CompletedTask;
            },
            TimeSpan.FromSeconds(1), default);

        Assert.Equal(new VidaaPictureSetting(2, "Laser Luminance", "5", "int", 1), Assert.Single(settings));
    }

    [Fact]
    public async Task StoresTheTokensAPairingIssues()
    {
        var configuration = new HisenseVidaaConfiguration();
        var router = new VidaaResponseRouter(configuration);
        var issued = router.TokenIssued.Expect();

        Deliver(router, TokenTopic,
            """
            {"accesstoken":"access","refreshtoken":"refresh",
             "accesstoken_time":1766974704,"refreshtoken_time":1766974705,
             "accesstoken_duration_day":5}
            """);

        Assert.True(await issued.WaitAsync(TimeSpan.FromSeconds(1)));
        Assert.Equal("access", configuration.AccessToken);
        Assert.Equal("refresh", configuration.RefreshToken);
        Assert.Equal(DateTimeOffset.FromUnixTimeSeconds(1766974704), configuration.AccessTokenIssuedAt);
        Assert.Equal(DateTimeOffset.FromUnixTimeSeconds(1766974705), configuration.RefreshTokenIssuedAt);
        Assert.Equal(5, configuration.AccessTokenDurationDays);
        // The device does not always state the refresh lifetime; the documented default holds.
        Assert.Equal(30, configuration.RefreshTokenDurationDays);
    }

    [Fact]
    public void KeepsTheStoredTokensWhenTheAnswerCarriesNone()
    {
        var configuration = new HisenseVidaaConfiguration { AccessToken = "access" };
        var router = new VidaaResponseRouter(configuration);
        var issued = router.TokenIssued.Expect();

        Deliver(router, TokenTopic, "{\"result\":0}");

        Assert.Equal("access", configuration.AccessToken);
        Assert.False(issued.IsCompleted);
    }

    [Fact]
    public async Task AcceptsTheEmptyRemoteNowAcknowledgement()
    {
        var router = new VidaaResponseRouter(new HisenseVidaaConfiguration());
        var accepted = router.PinAccepted.Expect();

        router.Handle(AuthenticationTopic, [], retained: false);

        Assert.True(await accepted.WaitAsync(TimeSpan.FromSeconds(1)));
    }

    [Fact]
    public async Task ReportsARejectedPin()
    {
        var router = new VidaaResponseRouter(new HisenseVidaaConfiguration());
        var accepted = router.PinAccepted.Expect();

        Deliver(router, AuthenticationTopic, "{\"result\":0}");

        var error = await Assert.ThrowsAsync<UnauthorizedAccessException>(
            () => accepted.WaitAsync(TimeSpan.FromSeconds(1)));
        Assert.Equal("The VIDAA PIN was rejected.", error.Message);
    }

    [Fact]
    public void LeavesACommandWaitingWhenTheAnswerIsMalformed()
    {
        var router = new VidaaResponseRouter(new HisenseVidaaConfiguration());
        var accepted = router.PinAccepted.Expect();

        Deliver(router, AuthenticationTopic, "not json");

        Assert.False(accepted.IsCompleted);
    }

    [Fact]
    public void IgnoresAnAnswerNobodyIsWaitingFor()
    {
        var router = new VidaaResponseRouter(new HisenseVidaaConfiguration());

        Deliver(router, VolumeTopic, "{\"volume_value\":37}");
        Deliver(router, "/remoteapp/mobile/broadcast/ui_service/state", "{\"statetype\":\"livetv\"}");
    }

    [Fact]
    public async Task StopsWaitingForAnAnswerThatNeverCame()
    {
        var router = new VidaaResponseRouter(new HisenseVidaaConfiguration());

        await Assert.ThrowsAsync<TimeoutException>(() => router.Volume.CollectAsync(
            _ => Task.CompletedTask, TimeSpan.FromMilliseconds(20), default));
    }

    [Fact]
    public async Task DoesNotLetALaterNotificationAnswerACommandNobodyAsked()
    {
        var router = new VidaaResponseRouter(new HisenseVidaaConfiguration());
        await router.Volume.CollectAsync(
            _ =>
            {
                Deliver(router, VolumeTopic, "{\"volume_value\":37}");
                return Task.CompletedTask;
            },
            TimeSpan.FromSeconds(1), default);

        // The device keeps broadcasting volume changes between commands.
        Deliver(router, VolumeTopic, "{\"volume_value\":11}");

        var volume = await router.Volume.CollectAsync(
            _ =>
            {
                Deliver(router, VolumeTopic, "{\"volume_value\":42}");
                return Task.CompletedTask;
            },
            TimeSpan.FromSeconds(1), default);
        Assert.Equal(42, volume);
    }
}
