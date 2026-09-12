using System.Buffers.Binary;
using System.IO.Pipes;
using System.Net.Sockets;
using System.Text;
using System.Text.Json;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Zoning;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The UI against an agent: one connection carrying requests, their answers, and the
/// events in between (v6 API). The agent here is a fake speaking the same frames, over
/// whichever transport this platform gives the real one — a Unix socket, a named pipe.
/// </summary>
public class AgentClientTests
{
    static readonly TimeSpan Patience = TimeSpan.FromSeconds(5);

    [Fact]
    public async Task SubscribesOnConnectingAndRaisesWhatTheAgentSays()
    {
        await using var agent = FakeAgent.Start();
        using var client = new AgentClient(agent.Endpoint);

        var states = new List<AgentState>();
        var connections = new List<bool>();
        var hookEvents = new List<LittleBigMouseServiceEventArgs>();
        var subscribed = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        client.StateChanged += (_, state) => { states.Add(state); subscribed.TrySetResult(); };
        client.ConnectionChanged += (_, up) => connections.Add(up);
        client.HookEventReceived += (_, e) => hookEvents.Add(e);

        client.Start();

        // The client subscribes itself; the answer carries the first state.
        var subscribe = await agent.NextRequestAsync();
        Assert.Equal("Subscribe", subscribe.GetProperty("Method").GetString());
        await agent.AnswerAsync(subscribe, AgentFrames.Snapshot(layoutId: "DELA0B1"));
        await subscribed.Task.WaitAsync(Patience);
        Assert.Equal("DELA0B1", states[0].LayoutId);
        Assert.Equal(LittleBigMouseEvent.Stopped, states[0].EngineEvent);
        Assert.Equal([true], connections);

        // Then every change, and what the hook said.
        await agent.EventAsync(AgentFrames.State("Running"));
        await agent.EventAsync(AgentFrames.Hook(LittleBigMouseEvent.Loaded, "2 zones (2 main)"));
        await WaitFor(() => states.Count == 2 && hookEvents.Count == 1);
        Assert.Equal(LittleBigMouseEvent.Running, states[1].EngineEvent);
        Assert.Equal(LittleBigMouseEvent.Loaded, hookEvents[0].Event);
        Assert.Equal("2 zones (2 main)", hookEvents[0].Payload);
    }

    [Fact]
    public async Task AnswersAreMatchedToTheirCallEvenWithEventsInBetween()
    {
        await using var agent = FakeAgent.Start();
        using var client = new AgentClient(agent.Endpoint);
        client.Start();
        await agent.AnswerAsync(await agent.NextRequestAsync(), AgentFrames.Snapshot());

        var seen = client.SeenProcessesAsync();
        var request = await agent.NextRequestAsync();
        Assert.Equal("SeenProcesses", request.GetProperty("Method").GetString());

        // An event, and an answer to something else, before the one being waited for.
        await agent.EventAsync(AgentFrames.State("Running"));
        await agent.EventAsync("""{"Id":99,"Result":null}""");
        await agent.AnswerAsync(request, """["/usr/bin/kate","/usr/bin/firefox"]""");

        Assert.Equal(["/usr/bin/kate", "/usr/bin/firefox"], await seen.WaitAsync(Patience));
    }

    [Fact]
    public async Task WhatTheAgentRefusesIsRaisedNotSwallowed()
    {
        await using var agent = FakeAgent.Start();
        using var client = new AgentClient(agent.Endpoint);
        client.Start();
        await agent.AnswerAsync(await agent.NextRequestAsync(), AgentFrames.Snapshot());

        var save = client.SaveLayoutAsync("SOMEONE_ELSE", new { });
        var request = await agent.NextRequestAsync();
        Assert.Equal("SaveLayout", request.GetProperty("Method").GetString());
        Assert.Equal("SOMEONE_ELSE", request.GetProperty("LayoutId").GetString());
        await agent.ErrorAsync(request, "the layout SOMEONE_ELSE is not the current one");

        var refused = await Assert.ThrowsAsync<AgentException>(() => save.WaitAsync(Patience));
        Assert.Contains("not the current one", refused.Message);
    }

    [Fact]
    public async Task AnAgentThatGoesAwayComesBack()
    {
        var agent = FakeAgent.Start();
        using var client = new AgentClient(agent.Endpoint);
        var connections = new List<bool>();
        client.ConnectionChanged += (_, up) => connections.Add(up);
        client.Start();
        await agent.AnswerAsync(await agent.NextRequestAsync(), AgentFrames.Snapshot("Running"));

        // A call in flight when the agent leaves fails, rather than hanging.
        var stopping = client.StopEngineAsync();
        await agent.NextRequestAsync();
        await agent.DisposeAsync();
        await Assert.ThrowsAnyAsync<Exception>(() => stopping.WaitAsync(Patience));
        await WaitFor(() => connections.Count >= 2 && !connections[^1]);

        // The next agent is connected to, and subscribed to, on its own.
        await using var next = FakeAgent.Start(agent.Endpoint);
        var subscribe = await next.NextRequestAsync();
        Assert.Equal("Subscribe", subscribe.GetProperty("Method").GetString());
        await WaitFor(() => connections[^1]);
    }

    [Fact]
    public async Task ACallMadeWithNoAgentThereCanBeGivenUpOn()
    {
        // The client waits for an agent rather than failing at once — it is usually only
        // being relaunched — so what keeps that from pinning the caller is the token.
        using var client = new AgentClient(
            Path.Combine(Path.GetTempPath(), $"lbm-agent-absent-{Guid.NewGuid():N}.sock"));
        client.Start();

        using var giveUp = new CancellationTokenSource(TimeSpan.FromMilliseconds(200));
        await Assert.ThrowsAnyAsync<OperationCanceledException>(
            () => client.StopEngineAsync(giveUp.Token).WaitAsync(Patience));
    }

    static Task WaitFor(Func<bool> done) => FakeAgent.WaitFor(done);
}
