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

    const string WindowsPipePrefix = @"\\.\pipe\";

    readonly CancellationTokenSource _stopping = new();

    // The event listener alone, so it can be shut down while the send path stays
    // open: a Quit is delivered through this very client, and the listener must
    // already be gone when the daemon it kills closes the socket.
    readonly CancellationTokenSource _listening;

    readonly SemaphoreSlim _sendGate = new(1, 1);
    readonly string _windowsPipeName;
    readonly string? _unixSocketPath;

    Task? _listener;

    // `LBM_HOOK_ENDPOINT` mirrors the daemon's override (the full pipe path on
    // Windows, the socket path elsewhere): a launched daemon inherits the
    // variable, so a test pair runs side by side with the production pair.
    public LocalIpcClient()
        : this(Environment.GetEnvironmentVariable("LBM_HOOK_ENDPOINT"))
    {
    }

    internal LocalIpcClient(string? endpoint)
    {
        _listening = CancellationTokenSource.CreateLinkedTokenSource(_stopping.Token);
        if (OperatingSystem.IsWindows())
        {
            _windowsPipeName = endpoint is null
                ? $"LittleBigMouse-v1-session-{Process.GetCurrentProcess().SessionId}"
                : PipeNameFromEndpoint(endpoint);
        }
        else
        {
            _windowsPipeName = string.Empty;
            _unixSocketPath = endpoint;
        }
    }

    // NamedPipeClientStream wants the bare pipe name while the daemon binds the
    // full path, so accept both spellings of the same endpoint.
    internal static string PipeNameFromEndpoint(string endpoint)
        => endpoint.StartsWith(WindowsPipePrefix, StringComparison.OrdinalIgnoreCase)
            ? endpoint[WindowsPipePrefix.Length..]
            : endpoint;

    public event EventHandler<string>? MessageReceived;
    public event EventHandler? Connected;
    public event EventHandler? ConnectionFailed;

    public void Listen() => _listener ??= Task.Run(ListenAsync);

    /// <summary>
    /// Give up on the daemon for good: no reconnect, and so no
    /// <see cref="ConnectionFailed"/> from the listener. Sends keep working — the
    /// caller still has a Quit to deliver, and it is exactly the daemon answering
    /// it whose disappearance must stop being reported as something to recover from.
    /// </summary>
    public void StopListening() => _listening.Cancel();

    async Task ListenAsync()
    {
        while (!_listening.IsCancellationRequested)
        {
            try
            {
                await using var stream = await ConnectWithRetryAsync(_listening.Token);
                Connected?.Invoke(this, EventArgs.Empty);
                await WriteFrameAsync(stream,
                    "<CommandMessage Command=\"Listen\" Payload=\"\"/>", _listening.Token);
                while (!_listening.IsCancellationRequested)
                {
                    var message = await ReadFrameAsync(stream, _listening.Token);
                    MessageReceived?.Invoke(this, message);
                }
            }
            catch (OperationCanceledException) when (_listening.IsCancellationRequested)
            {
                break;
            }
            catch (Exception error) when (error is IOException or SocketException
                                          or UnauthorizedAccessException or InvalidDataException)
            {
                // UnauthorizedAccessException (pipe ACL) and InvalidDataException
                // (bad frame) must reconnect like an I/O failure — an uncaught
                // exception here would kill the listener silently forever.
                MessageReceived?.Invoke(this,
                    "<DaemonMessage><Event>Dead</Event></DaemonMessage>");
                try { await Task.Delay(RetryDelay, _listening.Token); }
                catch (OperationCanceledException) { break; }
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

    // One attempt cycle is ~350 ms (250 ms connect + 100 ms delay): re-raising
    // ConnectionFailed every 14 attempts retries a daemon launch about every
    // 5 s. A one-shot notification would never relaunch a daemon that crashed
    // right after starting; LaunchDaemon's already-running check keeps the
    // periodic re-fire from ever spawning duplicates.
    const int NotifyEveryAttempts = 14;

    async Task<Stream> ConnectWithRetryAsync(CancellationToken token)
    {
        var attempts = 0;
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
                if (attempts++ % NotifyEveryAttempts == 0)
                    ConnectionFailed?.Invoke(this, EventArgs.Empty);
                await Task.Delay(RetryDelay, token);
            }
            catch (Exception error) when (error is IOException or SocketException
                                          or UnauthorizedAccessException)
            {
                if (attempts++ % NotifyEveryAttempts == 0)
                    ConnectionFailed?.Invoke(this, EventArgs.Empty);
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

        var socketPath = _unixSocketPath;
        if (socketPath is null)
        {
            var runtimeDir = Environment.GetEnvironmentVariable("XDG_RUNTIME_DIR");
            if (string.IsNullOrWhiteSpace(runtimeDir)) runtimeDir = LbmPaths.DataDir;
            socketPath = Path.Combine(runtimeDir, "littlebigmouse-v1.sock");
        }
        var socket = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        try
        {
            await socket.ConnectAsync(new UnixDomainSocketEndPoint(socketPath), token);
            return new NetworkStream(socket, ownsSocket: true);
        }
        catch
        {
            socket.Dispose();
            throw;
        }
    }

    internal static async Task<string> ReadFrameAsync(Stream stream, CancellationToken token)
    {
        var prefix = new byte[sizeof(uint)];
        await stream.ReadExactlyAsync(prefix, token);
        var length = BinaryPrimitives.ReadUInt32LittleEndian(prefix);
        if (length > MaxFrameSize) throw new InvalidDataException("IPC frame exceeds 1 MiB");
        var payload = new byte[checked((int)length)];
        await stream.ReadExactlyAsync(payload, token);
        try
        {
            return new UTF8Encoding(false, true).GetString(payload);
        }
        catch (DecoderFallbackException error)
        {
            // Same InvalidData class as an oversized frame: the listener's catch
            // filter must treat a corrupt peer like an I/O failure, not die on it.
            throw new InvalidDataException("IPC frame is not valid UTF-8", error);
        }
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
