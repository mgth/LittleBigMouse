using System;
using System.Threading;

#nullable enable
namespace HLab.Sys.Windows.MonitorVcp;

/// <summary>
/// Opt-in periodic refresh: while active, re-reads the VCP levels so changes
/// made on the monitor's own OSD show up in the sliders (typically during a
/// white-balance session). Bounded by a deadline so an abandoned panel never
/// polls the DDC bus forever.
/// </summary>
public sealed class LiveReadSession : IDisposable
{
    readonly Timer _timer;
    readonly Action _refresh;
    readonly Action<LiveReadSession> _stopped;
    long _remainingTicks;
    int _disposed;

    public LiveReadSession(Action refresh, Action<LiveReadSession> stopped,
        TimeSpan interval, TimeSpan duration)
    {
        _refresh = refresh;
        _stopped = stopped;
        _remainingTicks = Math.Max(1, duration.Ticks / interval.Ticks);
        _timer = new Timer(_ => Tick(), null, interval, interval);
    }

    void Tick()
    {
        if (Volatile.Read(ref _disposed) != 0) return;
        if (Interlocked.Decrement(ref _remainingTicks) < 0)
        {
            Dispose();
            return;
        }
        _refresh();
    }

    public void Dispose()
    {
        if (Interlocked.Exchange(ref _disposed, 1) != 0) return;
        _timer.Dispose();
        _stopped(this);
    }
}
