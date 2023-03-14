using HLab.Sys.Windows.Monitors;
using ReactiveUI;

namespace HLab.Sys.Windows.MonitorVcp
{
    public class MonitorLevel : ReactiveObject
    {
        readonly uint _component = 0;

        readonly VcpSetter _componentSetter = null;
        readonly VcpGetter _componentGetter = null;
        LevelParser _levelParser = null;
        MonitorDevice _monitor = null;

        public MonitorLevel(MonitorDevice monitor, LevelParser parser, VcpGetter getter, VcpSetter setter, uint component = 0)
        {
            _monitor = monitor;

            _component = component;
            _componentSetter = setter;
            _componentGetter = getter;

            _levelParser = parser;

            //_threadSetter = new LossyThread( );

            //_threadSetter.Add(GetValue);

            //H.Initialize(this);

            parser.Add(this);

        }


        public void SetToMax() { Value = Max; }

        public void SetToMin() { Value = Min; }


        public bool Moving
        {
            get => _moving;
            private set => this.RaiseAndSetIfChanged(ref _moving, value);
        }
        bool _moving;

        public bool Enabled
        {
            get => _enabled;
            private set => this.RaiseAndSetIfChanged(ref _enabled, value);
        }
        bool _enabled;


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
                            Moving = false;
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
                        using (DelayChangeNotifications())
                        {
                            Value = value;
                            Min = min;
                            Max = max;
                            Enabled = true;
                        }
                    }

                    retry = 0;
                }

                retry--;
            }
        }


        //private readonly LossyThread _threadSetter;

        uint _value;
        public uint Value
        {
            get => _value;
            set
            {
                if (_value == value) return;

                using (DelayChangeNotifications())
                {
                    this.RaiseAndSetIfChanged(ref _value, value);
                    Moving = true;
                    _levelParser.Add(this);
                }
            }
        }


        public uint Min
        {
            get => _min;
            private set => this.RaiseAndSetIfChanged(ref _min, value);
        }
        uint _min;

        public uint Max
        {
            get => _max;
            private set => this.RaiseAndSetIfChanged(ref _max, value);
        }
        uint _max;

    }
}