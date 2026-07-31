using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.Ui.Avalonia.Remote;

/// <summary>
/// Feeds the layout being edited to the running daemon, so resistances and geometry can
/// be felt with the real mouse instead of applied and undone. Nothing is persisted —
/// see <see cref="ILittleBigMouseClientService.SendLiveAsync"/>.
/// <para>
/// The rate is set by how often <see cref="TickAsync"/> is called; everything the user
/// does between two ticks collapses into a single send, which is the whole of the
/// buffering. A tick that would send what the daemon already has sends nothing, so a
/// still layout costs one <c>ComputeZones</c> and one serialization per tick and no IPC
/// at all. Dragging a monitor produces one send per tick instead of one per pixel.
/// </para>
/// <para>
/// The model is only read on the ticking thread (the UI thread in the app): zone
/// computation walks live reactive objects. What comes out is a detached snapshot —
/// <see cref="Zone"/> keeps the compiled links, not the monitor's
/// <c>BorderResistance</c> — so the send itself is free to finish on a worker.
/// </para>
/// </summary>
public sealed class LiveLayoutUpdater(
    Func<ZonesLayout?> compute,
    Func<ZonesLayout, CancellationToken, Task> send)
{
    /// <summary>
    /// How often the app ticks this. Short enough that adjusting a border feels
    /// immediate, long enough that a drag never turns into a burst of Load/Run pairs —
    /// each of those unhooks and re-hooks the daemon.
    /// </summary>
    public static readonly TimeSpan Interval = TimeSpan.FromMilliseconds(200);

    /// <summary>
    /// What the daemon was last told, as it went on the wire. Empty means "unknown",
    /// which makes the next tick send whatever the layout currently is.
    /// </summary>
    string _onTheWire = "";

    /// <summary>
    /// A send in flight. Ticks that land during one are dropped rather than queued: the
    /// next tick carries the latest layout anyway, and a stalled daemon must not build a
    /// backlog of geometries nobody wants any more.
    /// </summary>
    bool _sending;

    /// <summary>
    /// Forget what the daemon is believed to hold. Call it whenever that belief stops
    /// being true — a new layout instance, or the switch being turned back on after the
    /// daemon has been fed from somewhere else.
    /// </summary>
    public void Forget() => _onTheWire = "";

    /// <returns>True when this tick actually sent something.</returns>
    public async Task<bool> TickAsync(CancellationToken token = default)
    {
        if (_sending) return false;

        var zones = compute();
        if (zones is null) return false;

        var payload = zones.Serialize();
        if (payload == _onTheWire) return false;

        _sending = true;
        try
        {
            await send(zones, token);
            _onTheWire = payload;
            return true;
        }
        catch (Exception error) when (error is IOException
                                      or OperationCanceledException
                                      or UnauthorizedAccessException
                                      or InvalidOperationException)
        {
            // A live update is a convenience, never a correctness step: a daemon that is
            // gone, busy or slow just means this edit is not previewed. _onTheWire is
            // left untouched, so the next tick retries with whatever the layout is then.
            return false;
        }
        finally
        {
            _sending = false;
        }
    }
}
