#nullable enable
using System;
using System.Threading;

namespace LittleBigMouse.Ui.Avalonia.MonitorFrame;

/// <summary>
/// Owns the result of the latest asynchronous operation. Results from older
/// generations are rejected and disposed instead of replacing newer state.
/// </summary>
public sealed class LatestResourceSlot<T> : IDisposable where T : class
{
    readonly object _lock = new();
    long _generation;
    bool _disposed;
    T? _current;

    public long Begin() => Interlocked.Increment(ref _generation);

    public bool TryReplace(long generation, T? resource)
    {
        T? previous = null;
        var accepted = false;
        lock (_lock)
        {
            if (!_disposed && generation == Volatile.Read(ref _generation))
            {
                previous = _current;
                _current = resource;
                accepted = true;
                if (ReferenceEquals(previous, resource)) previous = null;
            }
            else if (ReferenceEquals(_current, resource))
            {
                resource = null;
            }
        }

        DisposeResource(accepted ? previous : resource);
        return accepted;
    }

    public void Dispose()
    {
        T? current;
        lock (_lock)
        {
            if (_disposed) return;
            _disposed = true;
            Interlocked.Increment(ref _generation);
            current = _current;
            _current = null;
        }
        DisposeResource(current);
    }

    static void DisposeResource(T? resource) => (resource as IDisposable)?.Dispose();
}
