namespace LittleBigMouse.Plugin.Vcp.Avalonia;

/// <summary>
/// Owns the lifetime of the single calibration operation allowed for a screen.
/// </summary>
internal sealed class CalibrationTaskCoordinator : IDisposable
{
    readonly object _sync = new();
    readonly CancellationTokenSource _lifetimeCancellation = new();
    readonly Action<string, Exception> _reportError;

    CancellationTokenSource? _activeCancellation;
    Action? _whenStopped;
    bool _disposed;

    public CalibrationTaskCoordinator(Action<string, Exception> reportError)
    {
        _reportError = reportError;
    }

    /// <summary>
    /// Starts an operation only when the coordinator is idle. Expected cancellation
    /// is consumed; other failures are reported and propagated to the caller.
    /// </summary>
    public async Task<bool> RunAsync(
        string operationName,
        Func<CancellationToken, Task> operation,
        CancellationToken cancellationToken = default)
    {
        CancellationTokenSource operationCancellation;

        lock (_sync)
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
            if (_activeCancellation is not null) return false;

            operationCancellation = CancellationTokenSource.CreateLinkedTokenSource(
                _lifetimeCancellation.Token,
                cancellationToken);
            _activeCancellation = operationCancellation;
        }

        try
        {
            await operation(operationCancellation.Token).ConfigureAwait(false);
            return true;
        }
        catch (OperationCanceledException) when (operationCancellation.IsCancellationRequested)
        {
            return true;
        }
        catch (Exception error)
        {
            _reportError(operationName, error);
            throw;
        }
        finally
        {
            Action? whenStopped = null;
            lock (_sync)
            {
                if (ReferenceEquals(_activeCancellation, operationCancellation))
                    _activeCancellation = null;

                if (_disposed)
                {
                    whenStopped = _whenStopped;
                    _whenStopped = null;
                }
            }

            operationCancellation.Dispose();
            whenStopped?.Invoke();
        }
    }

    public void Cancel()
    {
        lock (_sync)
            _activeCancellation?.Cancel();
    }

    public void Dispose() => Dispose(null);

    /// <summary>
    /// Cancels the active operation and invokes <paramref name="whenStopped"/>
    /// after it has left its calibration code (immediately when already idle).
    /// </summary>
    public void Dispose(Action? whenStopped)
    {
        Action? runNow = null;

        lock (_sync)
        {
            if (_disposed)
            {
                runNow = whenStopped;
            }
            else
            {
                _disposed = true;
                _whenStopped = whenStopped;
                _lifetimeCancellation.Cancel();
                _activeCancellation?.Cancel();

                if (_activeCancellation is null)
                {
                    runNow = _whenStopped;
                    _whenStopped = null;
                }
            }
        }

        runNow?.Invoke();
        _lifetimeCancellation.Dispose();
    }
}
