#nullable enable

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

/// <summary>
/// Serializes the commands a client sends over its single connection to one display.
///
/// Every network remote here multiplexes pairing, key presses and setting changes onto one
/// socket, and every one of those exchanges reads or replaces connection state — a token, a
/// session, a pending answer. Two commands overlapping would interleave writes on that socket
/// and let one command's reply satisfy the other's wait, so they are run one at a time rather
/// than merely made thread-safe.
///
/// <see cref="CloseAsync"/> lets the last command finish before the connection is torn down,
/// and turns every later command into <see cref="ObjectDisposedException"/> instead of a write
/// to a disposed socket.
/// </summary>
public sealed class DeviceCommandGate
{
    readonly SemaphoreSlim _gate = new(1, 1);
    bool _closed;

    /// <summary>Runs <paramref name="command"/> once no other command is in flight.</summary>
    /// <exception cref="ObjectDisposedException">The gate is closed.</exception>
    public async Task<T> RunExclusiveAsync<T>(
        Func<CancellationToken, Task<T>> command,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(command);

        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            ObjectDisposedException.ThrowIf(_closed, this);
            return await command(cancellationToken).ConfigureAwait(false);
        }
        finally
        {
            _gate.Release();
        }
    }

    /// <inheritdoc cref="RunExclusiveAsync{T}"/>
    public async Task RunExclusiveAsync(
        Func<CancellationToken, Task> command,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(command);

        await RunExclusiveAsync<object?>(async token =>
        {
            await command(token).ConfigureAwait(false);
            return null;
        }, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Waits for the command in flight, runs <paramref name="lastCommand"/> — closing the
    /// connection, as a rule — then refuses any further command. Calling it twice is harmless,
    /// so an owner may close the gate explicitly and still be disposed.
    /// </summary>
    public async Task CloseAsync(Func<Task> lastCommand)
    {
        ArgumentNullException.ThrowIfNull(lastCommand);
        if (Volatile.Read(ref _closed)) return;

        await _gate.WaitAsync().ConfigureAwait(false);
        try
        {
            if (_closed) return;
            Volatile.Write(ref _closed, true);
            await lastCommand().ConfigureAwait(false);
        }
        finally
        {
            _gate.Release();
            _gate.Dispose();
        }
    }
}
