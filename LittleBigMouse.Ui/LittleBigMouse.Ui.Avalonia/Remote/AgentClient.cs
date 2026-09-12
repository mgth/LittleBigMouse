#nullable enable
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Pipes;
using System.Net.Sockets;
using System.Security.Principal;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using LittleBigMouse.Plugins;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.Ui.Avalonia.Remote;

/// <summary>
/// The UI's way in to the agent (v6, phase 4): the agent owns the hook, the profiles and
/// the engine, and this is how a frontend asks for any of it.
/// <para>
/// One connection carries everything, unlike the hook's client where a command opened its
/// own: requests go out with an id, answers come back with it, and `State` and `Hook`
/// events arrive in between. The frames are the hook's (a little-endian length, then
/// UTF-8); what travels in them is JSON.
/// </para>
/// <para>
/// The connection is kept up: dropped, it is re-established, and the subscription is taken
/// again — so the UI sees the agent come and go rather than having to poll it. A call made
/// while there is no agent waits for one, up to its own timeout, rather than failing at
/// once: the agent is usually only being relaunched.
/// </para>
/// </summary>
public sealed class AgentClient : IDisposable, IAgentWallpaper
{
    const int MaxFrameSize = 1024 * 1024;
    static readonly TimeSpan RetryDelay = TimeSpan.FromMilliseconds(250);
    static readonly TimeSpan CallTimeout = TimeSpan.FromSeconds(5);

    static readonly JsonSerializerOptions Json = new()
    {
        PropertyNamingPolicy = null,
        DefaultIgnoreCondition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull,
    };

    readonly CancellationTokenSource _stopping = new();
    readonly SemaphoreSlim _sendGate = new(1, 1);
    readonly ConcurrentDictionary<long, TaskCompletionSource<JsonElement>> _pending = new();
    readonly string? _endpoint;

    Stream? _stream;
    Task? _listener;
    long _nextId;

    /// <param name="endpoint">
    /// The agent's endpoint: a socket path, or a Windows pipe (full path or bare name).
    /// The session's own by default — the name <c>lbm_ipc::endpoint</c> spells.
    /// </param>
    public AgentClient(string? endpoint = null) => _endpoint = endpoint;

    /// <summary>The agent's state, every time it changes (and once on connecting).</summary>
    public event EventHandler<AgentState>? StateChanged;

    /// <summary>
    /// What the hook said, forwarded by the agent under the names this UI already knows
    /// (the load outcome, the probe report, the rescue, a focused process).
    /// </summary>
    public event EventHandler<LittleBigMouseServiceEventArgs>? HookEventReceived;

    /// <summary>Whether an agent is answering.</summary>
    public event EventHandler<bool>? ConnectionChanged;

    public bool Connected => _stream is not null;

    /// <summary>The last state the agent published, if this client has one yet.</summary>
    public AgentState? State { get; private set; }

    /// <summary>Connects, and keeps connecting for as long as this client lives.</summary>
    public void Start() => _listener ??= Task.Run(ListenAsync);

    /// <summary>
    /// Waits for an agent to answer, up to <paramref name="patience"/>. False means none
    /// did — which is a question for the caller (start one?), not an error: the client
    /// goes on trying either way.
    /// </summary>
    public async Task<bool> WaitForConnectionAsync(TimeSpan patience,
        CancellationToken token = default)
    {
        var deadline = DateTime.UtcNow + patience;
        while (!Connected)
        {
            if (DateTime.UtcNow >= deadline) return false;
            try
            {
                await Task.Delay(TimeSpan.FromMilliseconds(50), token);
            }
            catch (OperationCanceledException)
            {
                return Connected;
            }
        }
        return true;
    }

    //==================//
    // The API          //
    //==================//

    /// <summary>Who is there: the agent's name, version and protocol.</summary>
    public Task<JsonElement> HelloAsync(CancellationToken token = default)
        => CallAsync("Hello", new Dictionary<string, object?> { ["Client"] = "LittleBigMouse.Ui" }, token);

    /// <summary>The user's Start (the tray's, or the window's without an edit).</summary>
    public Task StartEngineAsync(CancellationToken token = default)
        => CallAsync("Start", null, token);

    /// <summary>"Apply and start": the edit is applied, saved, and started.</summary>
    public Task StartEngineAsync(string layoutId, object document, CancellationToken token = default)
        => CallAsync("Start", Document(layoutId, document), token);

    public Task StopEngineAsync(CancellationToken token = default)
        => CallAsync("Stop", null, token);

    /// <summary>Rebuild the layout the automatic detection missed (#443).</summary>
    public Task RefreshAsync(CancellationToken token = default)
        => CallAsync("Refresh", null, token);

    /// <summary>The agent writes the layout; the UI never touches the store.</summary>
    public Task SaveLayoutAsync(string layoutId, object document, CancellationToken token = default)
        => CallAsync("SaveLayout", Document(layoutId, document), token);

    /// <summary>A live-preview tick: the hook runs the edit, nothing is saved.</summary>
    public Task PreviewAsync(string layoutId, object document, CancellationToken token = default)
        => CallAsync("Preview", Document(layoutId, document), token);

    /// <summary>The preview is over: the hook goes back to the current layout.</summary>
    public Task EndPreviewAsync(CancellationToken token = default)
        => CallAsync("EndPreview", null, token);

    /// <summary>The app-level options, the excluded list, the session autostart.</summary>
    public Task SaveOptionsAsync(object? options, IReadOnlyList<string>? excluded,
        bool? loadAtStartup, CancellationToken token = default)
        => CallAsync("SaveOptions", new Dictionary<string, object?>
        {
            ["Options"] = options,
            ["Excluded"] = excluded,
            ["LoadAtStartup"] = loadAtStartup,
        }, token);

    /// <summary>
    /// The wallpaper settings of one layout. The agent writes `wallpaper.json` and paints
    /// the desktop: a frontend edits, it does not apply (v6).
    /// </summary>
    public Task SaveWallpaperAsync(string layoutId, object settings)
        => CallAsync("SaveWallpaper", new Dictionary<string, object?>
        {
            ["LayoutId"] = layoutId,
            ["Settings"] = settings,
        });

    /// <summary>Ask the hook for its edge report; it comes back as a Probed event.</summary>
    public Task ProbeAsync(CancellationToken token = default)
        => CallAsync("Probe", null, token);

    /// <summary>The processes seen in the foreground this session, oldest first.</summary>
    public async Task<IReadOnlyList<string>> SeenProcessesAsync(CancellationToken token = default)
    {
        var result = await CallAsync("SeenProcesses", null, token);
        var seen = new List<string>();
        if (result.ValueKind == JsonValueKind.Array)
            foreach (var process in result.EnumerateArray())
                if (process.GetString() is { } name)
                    seen.Add(name);
        return seen;
    }

    /// <summary>Leave: the hook first, then the agent.</summary>
    public Task QuitAgentAsync(CancellationToken token = default)
        => CallAsync("Quit", null, token);

    static Dictionary<string, object?> Document(string layoutId, object document)
        => new() { ["LayoutId"] = layoutId, ["Document"] = document };

    /// <summary>
    /// One request, answered. The agent answers every request it understands; an error
    /// answer is raised as an <see cref="AgentException"/>, which is what the banner shows.
    /// </summary>
    public async Task<JsonElement> CallAsync(string method, IDictionary<string, object?>? arguments = null,
        CancellationToken token = default)
    {
        var id = Interlocked.Increment(ref _nextId);
        var request = new Dictionary<string, object?> { ["Id"] = id, ["Method"] = method };
        if (arguments is not null)
            foreach (var (name, value) in arguments)
                if (value is not null)
                    request[name] = value;

        var answer = new TaskCompletionSource<JsonElement>(TaskCreationOptions.RunContinuationsAsynchronously);
        _pending[id] = answer;
        try
        {
            using var budget = CancellationTokenSource.CreateLinkedTokenSource(token, _stopping.Token);
            budget.CancelAfter(CallTimeout);
            await SendAsync(JsonSerializer.Serialize(request, Json), budget.Token);
            return await answer.Task.WaitAsync(budget.Token);
        }
        finally
        {
            _pending.TryRemove(id, out _);
        }
    }

    //==================//
    // The connection   //
    //==================//

    async Task SendAsync(string frame, CancellationToken token)
    {
        await _sendGate.WaitAsync(token);
        try
        {
            // The agent may be starting, or coming back: wait for it rather than fail.
            while (_stream is null)
            {
                token.ThrowIfCancellationRequested();
                await Task.Delay(RetryDelay, token);
            }
            await WriteFrameAsync(_stream, frame, token);
        }
        finally
        {
            _sendGate.Release();
        }
    }

    async Task ListenAsync()
    {
        while (!_stopping.IsCancellationRequested)
        {
            try
            {
                await using var stream = await ConnectAsync(_stopping.Token);
                _stream = stream;
                ConnectionChanged?.Invoke(this, true);
                // The state now, then every change (the tray does the same in process).
                await WriteFrameAsync(stream, """{"Id":0,"Method":"Subscribe"}""", _stopping.Token);
                while (!_stopping.IsCancellationRequested)
                    Dispatch(await ReadFrameAsync(stream, _stopping.Token));
            }
            catch (OperationCanceledException) when (_stopping.IsCancellationRequested)
            {
                break;
            }
            catch (Exception error) when (error is IOException or SocketException
                                          or UnauthorizedAccessException or InvalidDataException
                                          or JsonException or OperationCanceledException)
            {
                // Whatever went wrong, an agent that is gone is not an error the UI can
                // act on: show it as disconnected and take the connection again.
            }
            finally
            {
                Disconnected();
            }

            try { await Task.Delay(RetryDelay, _stopping.Token); }
            catch (OperationCanceledException) { break; }
        }
    }

    void Disconnected()
    {
        if (_stream is null) return;
        _stream = null;
        State = null;
        foreach (var (id, pending) in _pending)
        {
            pending.TrySetException(new AgentException("the agent went away before answering"));
            _pending.TryRemove(id, out _);
        }
        ConnectionChanged?.Invoke(this, false);
    }

    /// <summary>
    /// A frame, applied as if an agent had sent it. The listener is the only caller in the
    /// application; a test speaks for an agent through it, over the real parsing, without
    /// a socket and without a thread.
    /// </summary>
    internal void Receive(string frame) => Dispatch(frame);

    void Dispatch(string frame)
    {
        using var document = JsonDocument.Parse(frame);
        var root = document.RootElement;

        if (root.TryGetProperty("Id", out var id) && id.TryGetInt64(out var callId))
        {
            // Id 0 is the subscription this client sends itself: its answer is the first
            // state, and nobody is waiting on it (the calls count from 1).
            if (callId == 0)
            {
                if (root.TryGetProperty("Result", out var first)) Raise(first);
                return;
            }
            if (!_pending.TryRemove(callId, out var pending)) return;
            if (root.TryGetProperty("Error", out var error))
                pending.TrySetException(new AgentException(error.GetString() ?? "the agent refused"));
            else if (root.TryGetProperty("Result", out var result))
                pending.TrySetResult(result.Clone());
            return;
        }

        if (!root.TryGetProperty("Event", out var kind)) return;
        switch (kind.GetString())
        {
            case "State" when root.TryGetProperty("State", out var state):
                Raise(state);
                break;
            case "Hook" when root.TryGetProperty("Hook", out var hook):
                var payload = root.TryGetProperty("Payload", out var text) ? text.GetString() : null;
                if (Enum.TryParse<LittleBigMouseEvent>(hook.GetString(), out var hookEvent))
                    HookEventReceived?.Invoke(this,
                        new LittleBigMouseServiceEventArgs(hookEvent, payload ?? ""));
                break;
        }
    }

    /// <summary>The subscription's own answer carries the first state, like an event.</summary>
    void Raise(JsonElement state)
    {
        if (state.Deserialize<AgentState>(Json) is not { } snapshot) return;
        State = snapshot;
        StateChanged?.Invoke(this, snapshot);
    }

    async Task<Stream> ConnectAsync(CancellationToken token)
    {
        while (true)
        {
            token.ThrowIfCancellationRequested();
            try
            {
                return await OpenAsync(token);
            }
            catch (Exception error) when (error is IOException or SocketException
                                          or UnauthorizedAccessException or TimeoutException)
            {
                await Task.Delay(RetryDelay, token);
            }
        }
    }

    async Task<Stream> OpenAsync(CancellationToken token)
    {
        if (OperatingSystem.IsWindows())
        {
            var pipe = new NamedPipeClientStream(".", WindowsPipeName(_endpoint), PipeDirection.InOut,
                PipeOptions.Asynchronous, TokenImpersonationLevel.Identification);
            try
            {
                using var attempt = CancellationTokenSource.CreateLinkedTokenSource(token);
                attempt.CancelAfter(TimeSpan.FromMilliseconds(250));
                await pipe.ConnectAsync(attempt.Token);
                return pipe;
            }
            catch (OperationCanceledException) when (!token.IsCancellationRequested)
            {
                await pipe.DisposeAsync();
                throw new TimeoutException("no agent on the pipe");
            }
            catch
            {
                await pipe.DisposeAsync();
                throw;
            }
        }

        var socket = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        try
        {
            await socket.ConnectAsync(new UnixDomainSocketEndPoint(_endpoint ?? SocketPath()), token);
            return new NetworkStream(socket, ownsSocket: true);
        }
        catch
        {
            socket.Dispose();
            throw;
        }
    }

    /// <summary>`lbm_ipc::endpoint::agent_socket_path`.</summary>
    internal static string SocketPath()
    {
        var runtimeDir = Environment.GetEnvironmentVariable("XDG_RUNTIME_DIR");
        if (string.IsNullOrWhiteSpace(runtimeDir)) runtimeDir = LbmPaths.DataDir;
        return Path.Combine(runtimeDir, "lbm-agent.sock");
    }

    /// <summary>`lbm_ipc::endpoint::agent_pipe_name`, as the client spells it.</summary>
    internal static string WindowsPipeName(string? endpoint)
    {
        if (endpoint is null)
            return $"LittleBigMouse-Agent-v1-session-{Process.GetCurrentProcess().SessionId}";
        const string prefix = @"\\.\pipe\";
        return endpoint.StartsWith(prefix, StringComparison.OrdinalIgnoreCase)
            ? endpoint[prefix.Length..]
            : endpoint;
    }

    internal static async Task<string> ReadFrameAsync(Stream stream, CancellationToken token)
    {
        var prefix = new byte[sizeof(uint)];
        await stream.ReadExactlyAsync(prefix, token);
        var length = System.Buffers.Binary.BinaryPrimitives.ReadUInt32LittleEndian(prefix);
        if (length > MaxFrameSize) throw new InvalidDataException("agent frame exceeds 1 MiB");
        var payload = new byte[checked((int)length)];
        await stream.ReadExactlyAsync(payload, token);
        try
        {
            return new System.Text.UTF8Encoding(false, true).GetString(payload);
        }
        catch (System.Text.DecoderFallbackException error)
        {
            throw new InvalidDataException("agent frame is not valid UTF-8", error);
        }
    }

    static async Task WriteFrameAsync(Stream stream, string frame, CancellationToken token)
    {
        var payload = System.Text.Encoding.UTF8.GetBytes(frame);
        if (payload.Length > MaxFrameSize) throw new InvalidDataException("agent frame exceeds 1 MiB");
        var prefix = new byte[sizeof(uint)];
        System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(prefix, (uint)payload.Length);
        await stream.WriteAsync(prefix, token);
        await stream.WriteAsync(payload, token);
        await stream.FlushAsync(token);
    }

    public void Dispose()
    {
        // Cancellation releases the pending I/O; the listener unwinds on its own, so
        // nothing else is disposed here (that would race its continuations).
        _stopping.Cancel();
    }
}

/// <summary>What the agent refused, or could not be asked.</summary>
public sealed class AgentException(string message) : Exception(message);
