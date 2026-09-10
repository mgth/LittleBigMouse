using System;
using System.Threading;
using System.Threading.Tasks;

namespace LittleBigMouse.Ui.Avalonia.Remote;

/// <summary>
/// Orders the requests that decide what the engine runs, and lets only the newest of
/// them reach the daemon.
/// <para>
/// A Start carries the zones computed when it was asked for, then waits — on a
/// topology prologue, a file write, the pipe — before the daemon sees them. Around a
/// display change several paths ask for one (the reconcile after a wake, the resume
/// watchdog, the rebuild), each with the layout of its own moment, and nothing ordered
/// them: a Start asked for before a rebuild could reach the daemon after the one asked
/// for after it, and the daemon would hook the desktop that had just gone away (#607).
/// Every request now takes a ticket on the way in; when its turn to send comes, a
/// ticket that is no longer the newest is dropped, because whatever superseded it is
/// closer to what the user has in front of them.
/// </para>
/// <para>
/// A Stop takes a ticket too — it supersedes any Start still on its way — but is never
/// dropped itself: the request to take the hook down is the one that must always land.
/// </para>
/// </summary>
public sealed class LatestRequestGate
{
    readonly SemaphoreSlim _turn = new(1, 1);
    int _latest;

    /// <summary>Register a request. The ticket says which one it is; any newer one makes it stale.</summary>
    public int Claim() => Interlocked.Increment(ref _latest);

    /// <summary>Is this ticket still the newest request?</summary>
    public bool IsLatest(int ticket) => ticket == Volatile.Read(ref _latest);

    /// <summary>
    /// Wait for the turn to send, then send only if no newer request was claimed in the
    /// meantime. Returns whether the send ran.
    /// </summary>
    public async Task<bool> SendIfLatestAsync(int ticket, Func<Task> send, CancellationToken token = default)
    {
        await _turn.WaitAsync(token);
        try
        {
            if (!IsLatest(ticket)) return false;
            await send();
            return true;
        }
        finally
        {
            _turn.Release();
        }
    }

    /// <summary>
    /// Wait for the turn to send, then send whatever the ticket. For the requests that
    /// must always land (Stop, Quit): they still queue behind a send in progress, so a
    /// Start that already passed the gate is followed, never overtaken.
    /// </summary>
    public async Task SendAsync(Func<Task> send, CancellationToken token = default)
    {
        await _turn.WaitAsync(token);
        try
        {
            await send();
        }
        finally
        {
            _turn.Release();
        }
    }
}
