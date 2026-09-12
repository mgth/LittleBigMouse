using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using System.Text.Json;
using System.Text.Json.Serialization;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins.Persistence;

namespace LittleBigMouse.Ui.Avalonia.Remote;

/// <summary>
/// Feeds the layout being edited to the agent, so resistances and geometry can be felt with
/// the real mouse instead of applied and undone. Nothing is persisted: a preview asks the
/// agent to run the document, not to write it.
/// <para>
/// The rate is set by how often <see cref="TickAsync"/> is called; everything the user
/// does between two ticks collapses into a single send, which is the whole of the
/// buffering. Dragging a monitor produces one send per tick instead of one per pixel.
/// </para>
/// <para>
/// Two gates, in order of cost. A revision counter — <c>SavableReactiveModel.Revision</c>,
/// bumped wherever the model marks itself unsaved — says whether anything at all has
/// moved; a tick over a still layout reads one number and stops there, which is what
/// makes leaving the switch on free. When it has moved, the document is built and compared
/// with what the agent was last given, so an edit that changes nothing the agent can see (a
/// value set back to itself, a repaint) still costs no IPC — and the agent is never made to
/// swap a layout for an identical one.
/// </para>
/// <para>
/// The model is only read on the ticking thread (the UI thread in the app): building the
/// document walks live reactive objects. It is handed to <c>send</c> on that same thread,
/// which serializes it before its first await — so what goes out is the geometry as it
/// stood when the send left, whatever the user does next.
/// </para>
/// </summary>
public sealed class LiveLayoutUpdater(
    Func<long> revision,
    Func<MonitorsLayout?> current,
    Func<MonitorsLayout, CancellationToken, Task> send)
{
    /// <summary>
    /// How often the app ticks this. Short enough that adjusting a border feels
    /// immediate, long enough that dragging a monitor never turns into a burst of
    /// layout swaps. This is a rate limit, not a poll: a tick with nothing to do costs
    /// one read of a counter.
    /// </summary>
    public static readonly TimeSpan Interval = TimeSpan.FromMilliseconds(200);

    /// <summary>
    /// What the agent was last told, as it went on the wire. Empty means "unknown",
    /// which makes the next tick send whatever the layout currently is.
    /// </summary>
    string _onTheWire = "";

    /// <summary>The document as the agent reads it (`lbm_store::LayoutDocument`).</summary>
    static readonly JsonSerializerOptions Wire = new()
    {
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
    };

    /// <summary>
    /// The model revision <see cref="_onTheWire"/> was read at. Below every real
    /// revision until a tick has looked at the model, so the first one always does.
    /// </summary>
    long _seenRevision = -1;

    /// <summary>
    /// A send in flight. Ticks that land during one are dropped rather than queued: the
    /// next tick carries the latest layout anyway, and a stalled agent must not build a
    /// backlog of geometries nobody wants any more.
    /// </summary>
    bool _sending;

    /// <summary>
    /// Forget what the agent is believed to hold. Call it whenever that belief stops
    /// being true — a new layout instance, or the switch being turned back on after the
    /// agent has been fed from somewhere else.
    /// </summary>
    public void Forget()
    {
        _onTheWire = "";
        _seenRevision = -1;
    }

    /// <returns>True when this tick actually sent something.</returns>
    public async Task<bool> TickAsync(CancellationToken token = default)
    {
        if (_sending) return false;

        // Read the revision before the model, never after: an edit landing while this
        // tick works must leave the next one with something to do.
        var seen = revision();
        if (seen == _seenRevision) return false;

        var layout = current();
        if (layout is null || layout.IsVirtual) return false;

        var payload = JsonSerializer.Serialize(AgentDocument.Of(layout), Wire);
        if (payload == _onTheWire)
        {
            // Something moved, but nothing the agent can see. Take the revision so the
            // document is not rebuilt until it moves again.
            _seenRevision = seen;
            return false;
        }

        _sending = true;
        try
        {
            await send(layout, token);
            _onTheWire = payload;
            _seenRevision = seen;
            return true;
        }
        catch (Exception error) when (error is IOException
                                      or OperationCanceledException
                                      or UnauthorizedAccessException
                                      or InvalidOperationException
                                      or AgentException)
        {
            // A live update is a convenience, never a correctness step: an agent that is
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
