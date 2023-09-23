using System;
using System.Collections.Concurrent;
using System.Threading.Tasks;

namespace HLab.Sys.Windows.MonitorVcp
{
    public sealed class LevelParser: IDisposable
    {
        Task _task;
        readonly ConcurrentQueue<MonitorLevel> _actions = new();

        public void Enqueue(MonitorLevel level)
        {
            _actions.Enqueue(level);
            _task ??= Task.Run(DoWork);
        }

        void DoWork()
        {
            var doWork = true;
            while (doWork)
            {
                if (_actions.TryDequeue(out var level))
                {
                    level.DoWork();
                }
                else
                {
                    doWork = false;
                }
            }
        }

        public void Dispose()
        {
                while (_actions.TryDequeue(out var l))
                {
                }
        }

    }
}