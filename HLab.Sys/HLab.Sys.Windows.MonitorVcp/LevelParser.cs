using System;
using System.Collections.Generic;
using System.Threading.Tasks;

namespace HLab.Sys.Windows.MonitorVcp
{
    public class LevelParser: IDisposable
    {
        private Task _task;

        public void Add(MonitorLevel level)
        {
            var shouldStart = false;
            lock (_lock)
            {
                if (_actions.Count == 0) shouldStart = true;
                if (_actions.Contains(level))
                    _actions.Remove(level);

                _actions.Insert(0,level);
            }

            if(shouldStart) _task = Task.Run(DoWork);
        }

        private readonly object _lock = new object();
        private readonly List<MonitorLevel> _actions = new List<MonitorLevel>();
        private void DoWork()
        {
            while(true)
            {
                MonitorLevel level;
                lock (_lock)
                {
                    if (_actions.Count == 0) return;
                    level = _actions[0];
                    _actions.Remove(level);
                    _actions.Add(level);
                }
                level.DoWork();
            }
        }
        public void Dispose()
        {
            lock (_lock)
            {
                while (_actions.Count > 0)
                    _actions.Remove(_actions[0]);
            }
        }

    }
}