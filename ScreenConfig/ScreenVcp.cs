using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using WinAPI_Dxva2;

namespace LbmScreenConfig
{
    public enum Component { Red, Green, Blue, Brightness, Contrast }
    public class ScreenVcp : INotifyPropertyChanged
    {
        // PropertyChanged Handling
        private readonly PropertyChangedHelper _change;
        public event PropertyChangedEventHandler PropertyChanged { add { _change.Add(this, value); } remove { _change.Remove(value); } }
        private Screen Screen { get; }
        public ScreenVcp(Screen screen)
        {
            _change = new PropertyChangedHelper(this);

            Screen = screen;
            Brightness = new MonitorLevel(GetBrightness, SetBrightness);
            Contrast = new MonitorLevel(GetContrast, SetContrast);

            Gain = new MonitorRgbLevel(GetGain, SetGain);
            Drive = new MonitorRgbLevel(GetDrive, SetDrive);

//            Probe = new MonitorRgbLevel(GetProbe, SetProbe);
        }


        public static DependencyProperty BrightnessProperty;
        public MonitorLevel Brightness { get; }
        public MonitorLevel Contrast { get; }

        public MonitorRgbLevel Gain { get; }
        public MonitorRgbLevel Drive { get; }
        public MonitorRgbLevel Probe { get; }

        public bool GetBrightness(ref uint min, ref uint value, ref uint max, uint component = 0)
    {
        return Dxva2.GetMonitorBrightness(Screen.HPhysical, ref min, ref value, ref max);
    }
    public bool SetBrightness(uint value, uint component = 0)
    {
        return Dxva2.SetMonitorBrightness(Screen.HPhysical, value);
    }
    public bool GetContrast(ref uint min, ref uint value, ref uint max, uint component = 0)
    {
        return Dxva2.GetMonitorContrast(Screen.HPhysical, ref min, ref value, ref max);
    }
    public bool SetContrast(uint value, uint component = 0)
    {
        return Dxva2.SetMonitorContrast(Screen.HPhysical, value);
    }
    public bool GetGain(ref uint min, ref uint value, ref uint max, uint component)
    {
        return Dxva2.GetMonitorRedGreenOrBlueGain(Screen.HPhysical, component, ref min, ref value, ref max);
    }
    public bool SetGain(uint value, uint component)
    {
        return Dxva2.SetMonitorRedGreenOrBlueGain(Screen.HPhysical, component, value);
    }
    public bool GetDrive(ref uint min, ref uint value, ref uint max, uint component)
    {
        return Dxva2.GetMonitorRedGreenOrBlueDrive(Screen.HPhysical, component, ref min, ref value, ref max);
    }
    public bool SetDrive(uint component, uint value)
    {
        return Dxva2.SetMonitorRedGreenOrBlueDrive(Screen.HPhysical, component, value);
    }

    //private ProbedColor _probedColor;
    //public ProbedColor ProbedColor
    //{
    //    get { return _probedColor; }
    //    set { _probedColor = value; changed("ProbedColor"); }
    //}
    //public bool GetProbe (ref uint min, ref uint value, ref uint max, uint component)
    //{
    //    return Dxva2.GetMonitorRedGreenOrBlueDrive(Screen.HPhysical, component, ref min, ref value, ref max);
    //}
    //public bool SetProbe(uint value, uint component)
    //{
    //    return Dxva2.SetMonitorRedGreenOrBlueDrive(Screen.HPhysical, component, value);
    //}
}

    public delegate bool VcpGetter( ref uint min, ref uint value, ref uint max, uint component = 0);
    public delegate bool VcpSetter( uint value, uint component = 0);

    public class MonitorRgbLevel : DependencyObject, INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private void changed(String name)
        {
            if (PropertyChanged != null) PropertyChanged(this, new PropertyChangedEventArgs(name));
        }

        private readonly MonitorLevel[] _values = new MonitorLevel[3];

        public MonitorRgbLevel(VcpGetter getter, VcpSetter setter)
        {
            for (uint i = 0; i < 3; i++)
                _values[i] = new MonitorLevel(getter, setter, i);

        }
        public MonitorLevel Channel(uint channel) { return _values[channel]; }

        public static DependencyProperty RedProperty;
        public MonitorLevel Red => Channel(0);
        public MonitorLevel Green => Channel(1);
        public MonitorLevel Blue => Channel(2);
    }

    public class MonitorLevel : INotifyPropertyChanged
    {
        // PropertyChanged Handling
        private readonly PropertyChangedHelper _change;
        public event PropertyChangedEventHandler PropertyChanged { add { _change.Add(this, value); } remove { _change.Remove(value); } }

        //Screen _screen;

        private uint _value = 0;
        private uint _min = 0;
        private uint _max = 0;
        private uint _pendding = uint.MaxValue;

        readonly uint _component = 0;

        private readonly VcpSetter _componentSetter = null;
        private readonly VcpGetter _componentGetter = null;


        public MonitorLevel(VcpGetter getter, VcpSetter setter, uint component = 0)
        {
            _change = new PropertyChangedHelper(this);
               //_screen = screen;
               _component = component;
            _componentSetter = setter;
            _componentGetter = getter;

            GetValueThread();
        }

        private static readonly object LockDdcCi = new object();

        private readonly object _lockPending = new object();

        private readonly object _lockTread = new object();

        public void SetToMax()
        {
            //GetValue();
            Value = Max;
        }

        public void SetToMin()
        {
            GetValue();
            Value = Min;
        }

        private void GetValue()
        {
            uint min = 0;
            uint max = 0;
            uint value = 0;

            lock(LockDdcCi)
            {
                _componentGetter?.Invoke(ref min, ref value, ref max, _component);
            }

            _change.SetProperty(ref _min, min, "Min");
            _change.SetProperty(ref _max, max, "Max");
            _change.SetProperty(ref _value, value, "Value");
        }
        private void GetValueThread()
        {
            Thread thread = new Thread(GetValue);
            thread.Start();
        }

        private Thread _threadSetter;
        private void SetValueThread()
        {
            //lock (_lockTread)
            {
                if (_threadSetter?.IsAlive??false) return;
                _threadSetter = new Thread(
                    delegate ()
                    {
                        // Only one DCC/CI request at a time
                        lock (LockDdcCi)
                        {
                            uint pendding = uint.MaxValue;
                            bool done = false;

                            while (!done)
                            {
                                lock (_lockPending)
                                {
                                    if (_pendding == pendding)
                                    {
                                        _pendding = uint.MaxValue;
                                        done = true;
                                    }
                                    else pendding = _pendding;
                                }

                                if (!done) _componentSetter?.Invoke(pendding, _component);
                            }
                            GetValue();
                        }
                    });

                _threadSetter.Start();                
            }

        }

        [DependsOn("Value")]
        public uint ValueAsync
        {
            get { return _value; }
            set
            {
                lock (_lockPending)
                {
                    if (_pendding == value) return;
                    _pendding = value;                   
                }
                SetValueThread();
            }
        }
        public uint Value
        {
            get
            {
                while (_pendding != uint.MaxValue)
                {
                    SetValueThread();
                }
                return _value;
            }
            set
            {
                lock (_lockPending)
                {
                    if (_pendding == value) return;
                    _pendding = value;
                }
                SetValueThread();
                //while (_pendding < uint.MaxValue);
            }
        }
        public uint Min => _min;

        public uint Max => _max;
    }
}
