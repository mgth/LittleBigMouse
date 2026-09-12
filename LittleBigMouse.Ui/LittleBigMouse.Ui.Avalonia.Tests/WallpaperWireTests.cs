using System.Text.Json;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugin.Wallpaper.Avalonia;
using LittleBigMouse.Ui.Avalonia.Remote;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// What the wallpaper editor hands the agent. The agent writes it to the very file this
/// side reads back (`wallpaper.json`), so the two have to agree on the shape down to how
/// each enum is spelled — and the two sides spell them differently on purpose, because
/// that is what <c>System.Text.Json</c> does with them.
/// </summary>
public class WallpaperWireTests
{
    /// <summary>
    /// The settings as they actually leave this process: through the client, onto the
    /// wire, as the agent reads them. Asserting on a serializer built here instead would
    /// pin a shape nobody sends.
    /// </summary>
    static async Task<JsonElement> SentAsync(LayoutWallpaperSettings settings)
    {
        await using var agent = FakeAgent.Start();
        using var client = new AgentClient(agent.Endpoint);
        client.Start();
        await agent.AnswerAsync(await agent.NextRequestAsync(), AgentFrames.Snapshot());

        var saving = client.SaveWallpaperAsync("DELA0B1", settings);
        var request = await agent.NextRequestAsync();
        Assert.Equal("SaveWallpaper", request.GetProperty("Method").GetString());
        Assert.Equal("DELA0B1", request.GetProperty("LayoutId").GetString());
        await agent.AnswerAsync(request, "null");
        await saving.WaitAsync(FakeAgent.Patience);
        return request.GetProperty("Settings");
    }

    [Fact]
    public async Task TheSettingsTravelAsTheFileSpellsThem()
    {
        var settings = new LayoutWallpaperSettings
        {
            Mode = WallpaperMode.Span,
            SpanImagePath = "/home/someone/desktop.png",
            PerScreen =
            {
                ["DELA0B1"] = new ScreenWallpaperSettings
                {
                    Kind = ScreenWallpaperKind.Color,
                    Style = WallpaperStyle.Fit,
                    Color = "#102030",
                },
            },
        };

        var root = await SentAsync(settings);

        Assert.Equal("Span", root.GetProperty("Mode").GetString());
        Assert.Equal("/home/someone/desktop.png", root.GetProperty("SpanImagePath").GetString());

        var screen = root.GetProperty("PerScreen").GetProperty("DELA0B1");
        // Annotated: a string. The agent reads "Fit", not 1.
        Assert.Equal("Fit", screen.GetProperty("Style").GetString());
        // Not annotated: a number, because that is what System.Text.Json writes for an
        // enum nobody gave a converter. The agent's own serde impl matches it by hand.
        Assert.Equal(1, screen.GetProperty("Kind").GetInt32());
        Assert.Equal("#102030", screen.GetProperty("Color").GetString());
    }

    [Fact]
    public async Task WhatIsNotSetIsNotSent()
    {
        // The agent leaves absent members as they are, so an empty span path must be
        // absent rather than null — the same rule the layout document follows.
        var settings = new LayoutWallpaperSettings { Mode = WallpaperMode.PerScreen };

        var sent = await SentAsync(settings);

        Assert.False(sent.TryGetProperty("SpanImagePath", out _));
    }

    [Fact]
    public async Task WhatIsSentIsWhatTheFileHolds()
    {
        // The agent writes what it is sent into wallpaper.json, and this side reads that
        // file back: a shape that survives the round trip is the whole contract.
        var settings = new LayoutWallpaperSettings
        {
            Mode = WallpaperMode.Span,
            SpanImagePath = "/pictures/wall.jpg",
            PerScreen = { ["MON1"] = new ScreenWallpaperSettings { Style = WallpaperStyle.Tile } },
        };

        var read = (await SentAsync(settings)).Deserialize<LayoutWallpaperSettings>();

        Assert.NotNull(read);
        Assert.Equal(WallpaperMode.Span, read.Mode);
        Assert.Equal("/pictures/wall.jpg", read.SpanImagePath);
        Assert.Equal(WallpaperStyle.Tile, read.PerScreen["MON1"].Style);
        Assert.Equal(ScreenWallpaperKind.Image, read.PerScreen["MON1"].Kind);
    }
}
