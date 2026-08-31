using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.CompilerServices;
using LittleBigMouse.Plugins;
using System.Threading;
using System.Threading.Tasks;
using Avalonia.Threading;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.Ui.Avalonia.Remote;

/// <summary>
/// Turns the UI's Start/Stop/live-preview intentions into daemon commands. It coordinates three
/// concerns, each owned by its own component:
/// <list type="bullet">
/// <item><see cref="LocalIpcClient"/> — the named-pipe / socket transport and reconnect loop.</item>
/// <item><see cref="DaemonProcessManager"/> — launching and force-stopping the hook process.</item>
/// <item><see cref="RecoveryStateStore"/> — the crash-recovery / autostart file.</item>
/// </list>
/// This class keeps the policy that ties them together: what a Start sends, when the topology
/// prologue runs, and how a lost-IPC Stop falls back to killing the process.
/// </summary>
public class LittleBigMouseClientService : ILittleBigMouseClientService, IDisposable
{
    public event EventHandler<LittleBigMouseServiceEventArgs>? DaemonEventReceived;
    readonly LocalIpcClient _client;
    readonly DaemonProcessManager _processManager;
    readonly RecoveryStateStore _recovery;

    protected void OnStateChanged(LittleBigMouseEvent evt, string payload = "")
    {
        if(evt<=LittleBigMouseEvent.Dead)
        {
            State = evt;
        }
        DaemonEventReceived?.Invoke(this, new (evt,payload));
    }

    readonly IDisplayController _displayController;
    readonly ILayoutOptions _options;

    public LittleBigMouseClientService(ILayoutOptions options, IDisplayController displayController)
        : this(options, displayController, new DaemonProcessManager(), new RecoveryStateStore())
    {
    }

    internal LittleBigMouseClientService(ILayoutOptions options, IDisplayController displayController,
        DaemonProcessManager processManager, RecoveryStateStore recovery)
    {
        _displayController = displayController;
        _options = options;
        _processManager = processManager;
        _recovery = recovery;
        _client = new LocalIpcClient();

        _client.ConnectionFailed += (sender, args) =>
        {
            Debug.WriteLine($"ConnectionFailed : Launch daemon");
            LaunchDaemon();
        };

        _client.MessageReceived += (sender, args) =>
        {
            if (!DaemonMessage.TryParse(args, out var message)) return;
            Dispatcher.UIThread.Post(() => OnStateChanged(message.Event, message.Payload));
        };

        _client.Connected += (sender, args) =>
        {
            Debug.WriteLine($"Connected");
            Dispatcher.UIThread.Post(() => OnStateChanged(LittleBigMouseEvent.Connected));
        };

        _client.Listen();
    }

    public LittleBigMouseEvent State { get; private set; } = LittleBigMouseEvent.Dead;


    public async Task StartAsync(ZonesLayout zonesLayout, CancellationToken token = default)
    {
        // Virtual layout: inspection only. Send Load WITHOUT Run — the daemon parses the
        // zones into its engine and reports Loaded/LoadFailed, but the input hook stays
        // down (it would refuse anyway). No topology prologue either: PrepareForEngine
        // MUTATES the local outputs, which a foreign layout must never cause.
        if (zonesLayout.Virtual)
        {
            await SendMessagesAsync([new CommandMessage(LittleBigMouseCommand.Load, zonesLayout)], token);
            return;
        }

        // Topology prologue (Linux/KWin: open 1px gaps so the daemon's barriers pass the
        // compositor validator; Windows: no-op, returns false). When it actually moves
        // outputs, the zones we were handed are stale by construction (computed in the
        // pre-gap space) — sending them would race the fresh ones and could win, arming
        // barriers in the wrong coordinate space. Drop this send: PrepareForEngine raised
        // DisplayChanged, MainService rebuilds and re-enters here with zones computed in
        // the gapped space (every start path runs with Options.Enabled set, which gates
        // that re-entry). Subprocess work: keep it off the UI thread.
        if (await Task.Run(_displayController.PrepareForEngine, token))
            return;

        var commands = new List<CommandMessage>()
        {
            new(LittleBigMouseCommand.Load, zonesLayout),
            new(LittleBigMouseCommand.Run)
        };

        await SendMessagesAsync(commands, token);
    }

    /// <summary>
    /// The live-preview send. Same two commands as a Start — a Load unhooks the daemon,
    /// so the Run is what puts the mouse back under the new geometry — with everything
    /// that makes a Start permanent left out: no store write, no recovery file, and no
    /// topology prologue (that one belongs to engine startup, and on Linux it forks a
    /// subprocess, which has no business running several times a second).
    /// </summary>
    public Task SendLiveAsync(ZonesLayout zonesLayout, CancellationToken token = default)
    {
        // A foreign layout is inspection-only; the daemon refuses to hook it anyway.
        if (zonesLayout.Virtual) return Task.CompletedTask;

        return SendMessagesAsync(
            [new(LittleBigMouseCommand.Load, zonesLayout), new(LittleBigMouseCommand.Run)],
            token, persist: false);
    }


    public Task SendShortcutAsync(string shortcut, CancellationToken token = default) =>
        SendMessagesAsync(
            [CommandMessage.WithText(LittleBigMouseCommand.Shortcut, shortcut ?? "")],
            token, persist: false);

    // The topology epilogue runs on explicit Stop/Quit only — NOT on a Dead daemon: the
    // socket layer auto-relaunches the daemon and MainService auto-restarts the engine
    // (Connected→Stopped→Start), so restoring there would fight the recovery. A daemon
    // that stays dead is covered by RecoverStale at next startup.
    public async Task StopAsync(CancellationToken token = default)
    {
        try
        {
            await SendAsync(token);
        }
        catch (Exception error) when (error is IOException
                                      or OperationCanceledException
                                      or UnauthorizedAccessException)
        {
            // Stop is a safety operation: losing IPC must not leave the input
            // hook active or fault ReactiveCommand's observable pipeline. Kill
            // only known hook images in this logon session as the last resort.
            var stopped = _processManager.StopCurrentSessionDaemons();
            Debug.WriteLine($"Stop IPC failed; daemon fallback stopped={stopped}: {error}");
            OnStateChanged(stopped ? LittleBigMouseEvent.Stopped : LittleBigMouseEvent.Dead);
        }
        await Task.Run(_displayController.RestoreAfterEngine, token);
    }

    public async Task QuitAsync(CancellationToken token = default)
    {
        await SendAsync(token);
        await Task.Run(_displayController.RestoreAfterEngine, token);
    }

    public void LaunchDaemon() => _processManager.LaunchDaemon();

    async Task StopDaemon(CancellationToken token = default)
    {
        await SendMessageAsync(new CommandMessage(LittleBigMouseCommand.Stop,null), _timeout, token);
    }

    readonly int _timeout = 5000;

    Task SendAsync(CancellationToken token = default, [CallerMemberName]string name = null)
    {
        if(name==null) throw new ArgumentNullException(nameof(name));
        if (name.EndsWith("Async")) name = name[..^5];
        return Enum.TryParse<LittleBigMouseCommand>(name, out var command) ? SendMessageAsync(
            new CommandMessage(command,null),_timeout,token) : Task.CompletedTask;
    }

    Task SendMessageAsync(CommandMessage message, int timeout,
        CancellationToken token = default)
    {
        return SendMessagesAsync([message], token, timeout);
    }

    /// <param name="persist">
    /// False for a send the daemon should run but not remember (live preview): the
    /// recovery file keeps describing the last applied layout. Serializing a layout is
    /// the expensive half of a send, so the recovery copy is only built when it is
    /// actually going to be written.
    /// </param>
    async Task SendMessagesAsync(IEnumerable<CommandMessage> messages,
        CancellationToken token = default, int timeout = 5000, bool persist = true)
    {
        var commands = messages.ToList();
        var wireXml = $"<Messages>{string.Concat(commands.Select(command => command.Serialize()))}</Messages>";

        var persistenceFailure = await _recovery.PersistAsync(commands, persist, token);

        await _client.SendMessageAsync(wireXml, TimeSpan.FromMilliseconds(timeout), token);
        if (persistenceFailure is not null)
            throw new InvalidOperationException(
                "The live configuration was applied, but crash-recovery settings could not be saved.",
                persistenceFailure);
    }

    public void Dispose()
    {
        _client.Dispose();
        _processManager.Dispose();
        GC.SuppressFinalize(this);
    }
}
