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
        await agent.AnswerAsync(subscribe, State("Stopped", layoutId: "DELA0B1"));
        await subscribed.Task.WaitAsync(Patience);
        Assert.Equal("DELA0B1", states[0].LayoutId);
        Assert.Equal(LittleBigMouseEvent.Stopped, states[0].EngineEvent);
        Assert.Equal([true], connections);

        // Then every change, and what the hook said.
        await agent.EventAsync($$"""{"Event":"State","State":{{State("Running")}}}""");
        await agent.EventAsync("""{"Event":"Hook","Hook":"Loaded","Payload":"2 zones (2 main)"}""");
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
        await agent.AnswerAsync(await agent.NextRequestAsync(), State("Stopped"));

        var seen = client.SeenProcessesAsync();
        var request = await agent.NextRequestAsync();
        Assert.Equal("SeenProcesses", request.GetProperty("Method").GetString());

        // An event, and an answer to something else, before the one being waited for.
        await agent.EventAsync($$"""{"Event":"State","State":{{State("Running")}}}""");
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
        await agent.AnswerAsync(await agent.NextRequestAsync(), State("Stopped"));

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
        await agent.AnswerAsync(await agent.NextRequestAsync(), State("Running"));

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

    static string State(string engine, string? layoutId = null) => $$"""
        {"AgentVersion":"0.1.0","HookConnected":true,"Engine":"{{engine}}","Suspended":false,
         "LayoutId":{{(layoutId is null ? "null" : $"\"{layoutId}\"")}},"Enabled":true,"Saved":true,
         "LoadAtStartup":false,"HideTrayIcon":false,"Previewing":false}
        """;

    static async Task WaitFor(Func<bool> done)
    {
        var deadline = DateTime.UtcNow + Patience;
        while (!done())
        {
            if (DateTime.UtcNow > deadline) Assert.Fail("the client never got there");
            await Task.Delay(20);
        }
    }

    /// <summary>An agent that decides nothing: it hands out what the test tells it to.</summary>
    sealed class FakeAgent : IAsyncDisposable
    {
        readonly CancellationTokenSource _stopping = new();
        readonly TaskCompletionSource<Stream> _connected =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        readonly Socket? _listener;
        readonly NamedPipeServerStream? _pipe;

        public string Endpoint { get; }

        FakeAgent(string endpoint, Socket? listener, NamedPipeServerStream? pipe)
        {
            Endpoint = endpoint;
            _listener = listener;
            _pipe = pipe;
            _ = Task.Run(AcceptAsync);
        }

        public static FakeAgent Start(string? endpoint = null)
        {
            if (OperatingSystem.IsWindows())
            {
                var name = endpoint ?? $"lbm-agent-test-{Guid.NewGuid():N}";
                return new FakeAgent(name,
                    null,
                    new NamedPipeServerStream(name, PipeDirection.InOut, 1,
                        PipeTransmissionMode.Byte, PipeOptions.Asynchronous));
            }

            var path = endpoint ?? Path.Combine(Path.GetTempPath(), $"lbm-agent-test-{Guid.NewGuid():N}.sock");
            File.Delete(path);
            var listener = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
            listener.Bind(new UnixDomainSocketEndPoint(path));
            listener.Listen(1);
            return new FakeAgent(path, listener, null);
        }

        async Task AcceptAsync()
        {
            try
            {
                if (_pipe is not null)
                {
                    await _pipe.WaitForConnectionAsync(_stopping.Token);
                    _connected.TrySetResult(_pipe);
                }
                else
                {
                    var accepted = await _listener!.AcceptAsync(_stopping.Token);
                    _connected.TrySetResult(new NetworkStream(accepted, ownsSocket: true));
                }
            }
            catch (Exception error)
            {
                _connected.TrySetException(error);
            }
        }

        async Task<Stream> StreamAsync() => await _connected.Task.WaitAsync(Patience);

        /// <summary>The next request the client sent.</summary>
        public async Task<JsonElement> NextRequestAsync()
        {
            var frame = await AgentClient.ReadFrameAsync(await StreamAsync(), _stopping.Token)
                .WaitAsync(Patience);
            return JsonDocument.Parse(frame).RootElement.Clone();
        }

        public Task AnswerAsync(JsonElement request, string result)
            => SendAsync($$"""{"Id":{{request.GetProperty("Id").GetInt64()}},"Result":{{result}}}""");

        public Task ErrorAsync(JsonElement request, string message)
            => SendAsync($$"""{"Id":{{request.GetProperty("Id").GetInt64()}},"Error":"{{message}}"}""");

        public Task EventAsync(string frame) => SendAsync(frame);

        async Task SendAsync(string frame)
        {
            var stream = await StreamAsync();
            var payload = Encoding.UTF8.GetBytes(frame);
            var prefix = new byte[sizeof(uint)];
            BinaryPrimitives.WriteUInt32LittleEndian(prefix, (uint)payload.Length);
            await stream.WriteAsync(prefix, _stopping.Token);
            await stream.WriteAsync(payload, _stopping.Token);
            await stream.FlushAsync(_stopping.Token);
        }

        public async ValueTask DisposeAsync()
        {
            await _stopping.CancelAsync();
            if (_connected.Task.IsCompletedSuccessfully) await _connected.Task.Result.DisposeAsync();
            _listener?.Dispose();
            if (_pipe is not null) await _pipe.DisposeAsync();
            if (!OperatingSystem.IsWindows()) File.Delete(Endpoint);
        }
    }
}
