using System.Buffers.Binary;
using System.IO.Pipes;
using System.Net.Sockets;
using System.Text;
using System.Text.Json;
using LittleBigMouse.Ui.Avalonia.Remote;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

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

    /// <summary>How long a test waits on something that travels through a socket.</summary>
    public static readonly TimeSpan Patience = TimeSpan.FromSeconds(5);

    /// <summary>Waits for what arrives on the client's own thread, or fails saying so.</summary>
    public static async Task WaitFor(Func<bool> done)
    {
        var deadline = DateTime.UtcNow + Patience;
        while (!done())
        {
            if (DateTime.UtcNow > deadline) Assert.Fail("the client never got there");
            await Task.Delay(20);
        }
    }
}
