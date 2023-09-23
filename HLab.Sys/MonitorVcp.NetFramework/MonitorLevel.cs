using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using HLab.Notify.PropertyChanged;

namespace HLab.Windows.MonitorVcp
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

    public class MonitorLevel : N<MonitorLevel>
    {
        private readonly uint _component = 0;

        private readonly VcpSetter _componentSetter = null;
        private readonly VcpGetter _componentGetter = null;
        private LevelParser _levelParser = null;
        private Monitors.Monitor _monitor = null;

        public MonitorLevel(Monitors.Monitor monitor, LevelParser parser, VcpGetter getter, VcpSetter setter, uint component = 0)
        {
            _monitor = monitor;

            _component = component;
            _componentSetter = setter;
            _componentGetter = getter;

            _levelParser = parser;

            //_threadSetter = new LossyThread( );

            //_threadSetter.Add(GetValue);

            Initialize();

            parser.Add(this);

        }


        public void SetToMax() { Value = Max; }

        public void SetToMin() { Value = Min; }


        private readonly IProperty<bool> _moving = H.Property<bool>();
        public bool Moving => _moving.Get();

        private readonly IProperty<bool> _enabled = H.Property<bool>();
        public bool Enabled => _enabled.Get();
        internal void DoWork()
        {
            uint min = 0;
            uint max = 0;
            uint value = 0;

            var retry = (!Enabled || Moving) ?10:1;

            while (retry > 0)
            {
                if (_componentGetter(ref min, ref value, ref max, _component))
                {
                    if (Moving)
                    {
                        if (value == Value)
                        {
                            _moving.Set(false);
                        }
                        else
                        {
                            while (retry > 0)
                            {
                                if (_componentSetter(Value, _component))
                                {
                                    retry = 0;
                                }
                                else retry--;
                            }
                        }
                    }
                    else
                    {
                        _value.Set(value);
                        _min.Set(min);
                        _max.Set(max);
                        _enabled.Set(true);
                    }

                    retry = 0;
                }

                retry--;
            }
        }


        //private readonly LossyThread _threadSetter;

        private readonly IProperty<uint> _value = H.Property<uint>(nameof(Value));
        public uint Value
        {
            get => _value.Get(); set => _value.Set(value,
                ()=>
                {
                    _moving.Set(true);
                    _levelParser.Add(this);
                });
        }


        public uint Min
        {
            get => _min.Get();
            private set => _min.Set(value);
        }
        private readonly IProperty<uint> _min = H.Property<uint>(nameof(Min));

        public uint Max
        {
            get => _max.Get();
            private set => _max.Set(value);
        }
        private readonly IProperty<uint> _max = H.Property<uint>(nameof(Max));

    }
}