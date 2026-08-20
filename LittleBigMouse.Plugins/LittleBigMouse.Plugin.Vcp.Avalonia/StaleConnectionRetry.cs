#nullable enable

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

/// <summary>
/// Sends one command over a session that has been sitting idle, and gives it a second chance
/// when the send is what reveals the session died meanwhile.
///
/// These displays keep one connection open between commands, and nothing tells the client when
/// the other end drops it: the television goes to sleep, the projector moves to another
/// network, a router forgets the flow. <c>TcpClient.Connected</c> and
/// <c>WebSocketState.Open</c> both stay true until the first failed write, so that write is the
/// probe — a transport failure on the first attempt means the session was stale, not that the
/// display refused the command. Reporting it would show the user a network error for a key
/// press that was never delivered anywhere.
///
/// Exactly one retry, and only after reopening. If the second attempt fails too, the display
/// really is unreachable and that second failure is the one worth showing.
/// </summary>
public static class StaleConnectionRetry
{
    /// <param name="send">
    /// Writes the command. Called once, or twice when the first call finds a dead session, so
    /// it must be safe to repeat — which it is here, since a command lost with the session
    /// never reached the display. It must also read the connection when it runs rather than
    /// close over one, because <paramref name="reopen"/> replaces it between the two calls.
    /// </param>
    /// <param name="reopen">Drops the stale session and establishes a new one.</param>
    /// <param name="isTransportFailure">
    /// Tells a dead session from a refused command. Left to the caller because only it knows
    /// what its transport raises, and because taking a protocol error for a dead socket would
    /// send the command a second time for nothing.
    /// </param>
    public static async Task SendAsync(
        Func<CancellationToken, Task> send,
        Func<CancellationToken, Task> reopen,
        Func<Exception, bool> isTransportFailure,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(send);
        ArgumentNullException.ThrowIfNull(reopen);
        ArgumentNullException.ThrowIfNull(isTransportFailure);

        try
        {
            await send(cancellationToken).ConfigureAwait(false);
            return;
        }
        catch (Exception exception) when (isTransportFailure(exception))
        {
            // The session was stale. Reopen and send again below, outside the catch block, so
            // a failure of the retry is reported on its own rather than while handling another.
        }

        await reopen(cancellationToken).ConfigureAwait(false);
        await send(cancellationToken).ConfigureAwait(false);
    }
}
