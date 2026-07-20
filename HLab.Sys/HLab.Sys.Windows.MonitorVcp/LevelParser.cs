using System;
using System.Collections.Concurrent;
using System.Threading.Tasks;

#nullable enable
namespace HLab.Sys.Windows.MonitorVcp;

public sealed class CommandWorker
{
    readonly object _gate = new();
    Task? _task;
    readonly ConcurrentQueue<IWorkProvider> _actions = new();
    bool _stop;

    public int PendingCount => _actions.Count;

    public void Enqueue(IWorkProvider level)
    {
        ArgumentNullException.ThrowIfNull(level);
        lock (_gate)
        {
            _stop = false;
            _actions.Enqueue(level);
            _task ??= Task.Run(DoWork);
        }
    }

    void DoWork()
    {
        while (true)
        {
            IWorkProvider? level;
            lock (_gate)
            {
                if (_stop)
                {
                    while (_actions.TryDequeue(out _)) { }
                    _stop = false;
                    _task = null;
                    return;
                }

                if (!_actions.TryDequeue(out level))
                {
                    _task = null;
                    return;
                }
            }

            level.DoWork();
        }
    }

    public void Stop()
    {
        lock (_gate) _stop = true;
    }
}
