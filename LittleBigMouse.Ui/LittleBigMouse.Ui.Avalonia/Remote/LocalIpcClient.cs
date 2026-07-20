using System;
using System.Buffers.Binary;
using System.Diagnostics;
using System.IO;
using System.IO.Pipes;
using System.Net.Sockets;
using System.Security.Principal;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Ui.Avalonia.Remote;

/// <summary>Bounded per-user local IPC client shared by command and event paths.</summary>
sealed class LocalIpcClient : IDisposable
{
    const int MaxFrameSize = 1024 * 1024;
    static readonly TimeSpan ConnectAttemptTimeout = TimeSpan.FromMilliseconds(250);
    static readonly TimeSpan RetryDelay = TimeSpan.FromMilliseconds(100);

    readonly CancellationTokenSource _stopping = new();
    readonly SemaphoreSlim _sendGate = new(1, 1);
    readonly string _windowsPipeName;
    Task? _listener;

    public LocalIpcClient()
        : this(OperatingSystem.IsWindows()
            ? $"LittleBigMouse-v1-session-{Process.GetCurrentProcess().SessionId}"
            : string.Empty)
    {
    }

    internal LocalIpcClient(string windowsPipeName)
    {
        _windowsPipeName = windowsPipeName;
    }

    public event EventHandler<string>? MessageReceived;
    public event EventHandler? Connected;
    public event EventHandler? ConnectionFailed;

    public void Listen() => _listener ??= Task.Run(ListenAsync);

    async Task ListenAsync()
    {
        while (!_stopping.IsCancellationRequested)
        {
            try
            {
                await using var stream = await ConnectWithRetryAsync(_stopping.Token);
                Connected?.Invoke(this, EventArgs.Empty);
                await WriteFrameAsync(stream,
                    "<CommandMessage Command=\"Listen\" Payload=\"\"/>", _stopping.Token);
                while (!_stopping.IsCancellationRequested)
                {
                    var message = await ReadFrameAsync(stream, _stopping.Token);
                    MessageReceived?.Invoke(this, message);
                }
            }
            catch (OperationCanceledException) when (_stopping.IsCancellationRequested)
            {
                break;
            }
            catch (Exception error) when (error is IOException or SocketException)
            {
                MessageReceived?.Invoke(this,
                    "<DaemonMessage><Event>Dead</Event></DaemonMessage>");
            }
        }
    }

    public async Task SendMessageAsync(string message, TimeSpan timeout,
        CancellationToken cancellationToken)
    {
        using var budget = CancellationTokenSource.CreateLinkedTokenSource(
            cancellationToken, _stopping.Token);
        budget.CancelAfter(timeout);
        await _sendGate.WaitAsync(budget.Token);
        try
        {
            await using var stream = await ConnectWithRetryAsync(budget.Token);
            await WriteFrameAsync(stream, message, budget.Token);
        }
        finally
        {
            _sendGate.Release();
        }
    }

    async Task<Stream> ConnectWithRetryAsync(CancellationToken token)
    {
        var notified = false;
        while (true)
        {
            token.ThrowIfCancellationRequested();
            using var attempt = CancellationTokenSource.CreateLinkedTokenSource(token);
            attempt.CancelAfter(ConnectAttemptTimeout);
            try
            {
                return await ConnectAsync(attempt.Token);
            }
            catch (OperationCanceledException) when (!token.IsCancellationRequested)
            {
                // NamedPipeClientStream waits for a server instead of reporting
                // "not found". Bound each attempt so the service can launch or
                // relaunch the daemon through ConnectionFailed.
                if (!notified)
                {
                    notified = true;
                    ConnectionFailed?.Invoke(this, EventArgs.Empty);
                }
                await Task.Delay(RetryDelay, token);
            }
            catch (Exception error) when (error is IOException or SocketException)
            {
                if (!notified)
                {
                    notified = true;
                    ConnectionFailed?.Invoke(this, EventArgs.Empty);
                }
                await Task.Delay(RetryDelay, token);
            }
        }
    }

    async Task<Stream> ConnectAsync(CancellationToken token)
    {
        if (OperatingSystem.IsWindows())
        {
            var pipe = new NamedPipeClientStream(".", _windowsPipeName, PipeDirection.InOut,
                PipeOptions.Asynchronous, TokenImpersonationLevel.Identification);
            try
            {
                await pipe.ConnectAsync(token);
                return pipe;
            }
            catch
            {
                await pipe.DisposeAsync();
                throw;
            }
        }

        var endpoint = Environment.GetEnvironmentVariable("XDG_RUNTIME_DIR");
        if (string.IsNullOrWhiteSpace(endpoint)) endpoint = LbmPaths.DataDir;
        var socket = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        try
        {
            await socket.ConnectAsync(
                new UnixDomainSocketEndPoint(Path.Combine(endpoint, "littlebigmouse-v1.sock")),
                token);
            return new NetworkStream(socket, ownsSocket: true);
        }
        catch
        {
            socket.Dispose();
            throw;
        }
    }

    static async Task<string> ReadFrameAsync(Stream stream, CancellationToken token)
    {
        var prefix = new byte[sizeof(uint)];
        await stream.ReadExactlyAsync(prefix, token);
        var length = BinaryPrimitives.ReadUInt32LittleEndian(prefix);
        if (length > MaxFrameSize) throw new InvalidDataException("IPC frame exceeds 1 MiB");
        var payload = new byte[checked((int)length)];
        await stream.ReadExactlyAsync(payload, token);
        return new UTF8Encoding(false, true).GetString(payload);
    }

    static async Task WriteFrameAsync(Stream stream, string message, CancellationToken token)
    {
        var payload = Encoding.UTF8.GetBytes(message);
        if (payload.Length > MaxFrameSize) throw new InvalidDataException("IPC frame exceeds 1 MiB");
        var prefix = new byte[sizeof(uint)];
        BinaryPrimitives.WriteUInt32LittleEndian(prefix, (uint)payload.Length);
        await stream.WriteAsync(prefix, token);
        await stream.WriteAsync(payload, token);
        await stream.FlushAsync(token);
    }

    public void Dispose()
    {
        // Cancellation releases pending I/O. The listener may still be unwinding,
        // so disposing its token source or the send gate here would introduce a
        // shutdown race with those continuations.
        _stopping.Cancel();
    }
}
