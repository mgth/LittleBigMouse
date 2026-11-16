using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Avalonia.Remote;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// "Apply to system" saves through the agent first: the display change triggers a rebuild
/// that reloads the saved layout. The editor stays usable while the agent answers, so the
/// save only counts if what the agent wrote is still the layout on screen.
/// </summary>
public sealed class ApplyLayoutSaveTests
{
    [Fact]
    public async Task ASaveTheAgentAcceptsMarksTheLayoutSaved()
    {
        await using var agent = FakeAgent.Start();
        using var client = new AgentClient(agent.Endpoint);
        client.Start();
        await agent.AnswerAsync(await agent.NextRequestAsync(), AgentFrames.Snapshot("Running"));

        var layout = MainServiceFakes.NewLayout(new LbmOptions());
        layout.Saved = false;

        var saving = MonitorsLayoutPresenterViewModel.SaveThroughAgentAsync(client, layout);
        var request = await agent.NextRequestAsync();
        Assert.Equal("SaveLayout", request.GetProperty("Method").GetString());
        await agent.AnswerAsync(request, "null");

        Assert.Null(await saving.WaitAsync(FakeAgent.Patience));
        Assert.True(layout.Saved);
    }

    [Fact]
    public async Task AnEditMadeWhileTheAgentAnswersIsNotMarkedSaved()
    {
        // The review's sequence: 200 is sent, the user types 999 before the reply, the
        // agent acknowledges the 200. Marking the layout saved would let Apply rebuild
        // from the 200 and drop the 999 without a word.
        await using var agent = FakeAgent.Start();
        using var client = new AgentClient(agent.Endpoint);
        client.Start();
        await agent.AnswerAsync(await agent.NextRequestAsync(), AgentFrames.Snapshot("Running"));

        var layout = MainServiceFakes.NewLayout(new LbmOptions());
        layout.Options.MaxTravelDistance = 200;
        layout.Saved = false;

        var saving = MonitorsLayoutPresenterViewModel.SaveThroughAgentAsync(client, layout);
        var request = await agent.NextRequestAsync();
        Assert.Contains("200", request.GetRawText());
        layout.Options.MaxTravelDistance = 999;
        await agent.AnswerAsync(request, "null");

        Assert.Equal("the layout changed while it was being saved",
            await saving.WaitAsync(FakeAgent.Patience));
        Assert.False(layout.Saved, "the edit is still unsaved, and Apply does not proceed");
    }

    [Fact]
    public async Task ARefusalIsReportedAndLeavesTheLayoutUnsaved()
    {
        await using var agent = FakeAgent.Start();
        using var client = new AgentClient(agent.Endpoint);
        client.Start();
        await agent.AnswerAsync(await agent.NextRequestAsync(), AgentFrames.Snapshot("Running"));

        var layout = MainServiceFakes.NewLayout(new LbmOptions());
        layout.Saved = false;

        var saving = MonitorsLayoutPresenterViewModel.SaveThroughAgentAsync(client, layout);
        await agent.ErrorAsync(await agent.NextRequestAsync(), "the layout TESTMON1 is not the current one");

        Assert.Contains("not the current one", await saving.WaitAsync(FakeAgent.Patience));
        Assert.False(layout.Saved);
    }

    [Fact]
    public async Task NoAgentMeansNothingIsApplied()
    {
        var layout = MainServiceFakes.NewLayout(new LbmOptions());
        layout.Saved = false;

        Assert.Equal("no agent", await MonitorsLayoutPresenterViewModel.SaveThroughAgentAsync(null, layout));
        Assert.False(layout.Saved);
    }
}
