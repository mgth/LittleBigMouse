using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using LbmScreenConfig;
using WinAPI_Dxva2;

namespace LittleBigMouse
{
   public class MonitorControl : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private void Changed(string name)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
        }

        private void Changed(object o, string name)
        {
            PropertyChanged?.Invoke(o, new PropertyChangedEventArgs(name));
        }
        public Screen Screen { get; }

        public MonitorControl(Screen screen)
        {
            Screen = screen;

            Brightness = new MonitorLevel(GetBrightness, SetBrightness);
            Contrast = new MonitorLevel(GetContrast, SetContrast);

            Gain = new MonitorRGBLevel(GetGain, SetGain);
            Drive = new MonitorRGBLevel(GetDrive, SetDrive);

            Probe = new MonitorRGBLevel(GetProbe, SetProbe);
        }

        public MonitorLevel Brightness { get; }
        public MonitorLevel Contrast { get; }
        public MonitorRGBLevel Gain { get; }
        public MonitorRGBLevel Drive { get; }
        public MonitorRGBLevel Probe { get; }

        public bool GetBrightness(ref uint min, ref uint value, ref uint max)
        {
            if (Screen==null) return false;
            return Dxva2.GetMonitorBrightness(Screen.HPhysical, ref min, ref value, ref max);
        }
        public bool SetBrightness(uint value)
        {
            if (Screen == null) return false;
            return Dxva2.SetMonitorBrightness(Screen.HPhysical, value);
        }
        public bool GetContrast(ref uint min, ref uint value, ref uint max)
        {
            if (Screen == null) return false;
            return Dxva2.GetMonitorContrast(Screen.HPhysical, ref min, ref value, ref max);
        }
        public bool SetContrast(uint value)
        {
            if (Screen == null) return false;
            return Dxva2.SetMonitorContrast(Screen.HPhysical, value);
        }
        public bool GetGain(uint component, ref uint min, ref uint value, ref uint max)
        {
            if (Screen == null) return false;
            return Dxva2.GetMonitorRedGreenOrBlueGain(Screen.HPhysical, component, ref min, ref value, ref max);
        }
        public bool SetGain( uint component, uint value)
        {
            if (Screen == null) return false;
            return Dxva2.SetMonitorRedGreenOrBlueGain(Screen.HPhysical, component, value);
        }
        public bool GetDrive(uint component, ref uint min, ref uint value, ref uint max)
        {
            if (Screen == null) return false;
            return Dxva2.GetMonitorRedGreenOrBlueDrive(Screen.HPhysical, component, ref min, ref value, ref max);
        }
        public bool SetDrive(uint component, uint value)
        {
            if (Screen == null) return false;
            return Dxva2.SetMonitorRedGreenOrBlueDrive(Screen.HPhysical, component, value);
        }

        private ProbedColor _probedColor;
        public ProbedColor ProbedColor
        {
            get { return _probedColor; }
            set { _probedColor = value; Changed("ProbedColor"); }
        }
        public bool GetProbe(uint component, ref uint min, ref uint value, ref uint max)
        {
            if (Screen == null) return false;
            return Dxva2.GetMonitorRedGreenOrBlueDrive(Screen.HPhysical, component, ref min, ref value, ref max);
        }
        public bool SetProbe(uint component, uint value)
        {
            if (Screen == null) return false;
            return Dxva2.SetMonitorRedGreenOrBlueDrive(Screen.HPhysical, component, value);
        }
    }

    public delegate bool MonitorGetter(ref uint min, ref uint value, ref uint max);
    public delegate bool MonitorComponentGetter(uint component, ref uint min, ref uint value, ref uint max);
    public delegate bool MonitorSetter(uint value);
    public delegate bool MonitorComponentSetter(uint component, uint value);

    public class MonitorRGBLevel : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private void changed(String name)
        {
            if (PropertyChanged != null) PropertyChanged(this, new PropertyChangedEventArgs(name));
        }

        private MonitorLevel[] _values = new MonitorLevel[3];

        public MonitorRGBLevel(MonitorComponentGetter getter, MonitorComponentSetter setter)
        {
            for (uint i = 0; i < 3; i++)
                _values[i] = new MonitorLevel(getter, setter, i);

        }
        public MonitorLevel Channel(uint channel) { return _values[channel]; }
        public MonitorLevel Red => Channel(0);
        public MonitorLevel Green => Channel(1);
        public MonitorLevel Blue => Channel(2);
    }
    public class MonitorLevel : INotifyPropertyChanged
    {
        static private int _locked = 0;

        public event PropertyChangedEventHandler PropertyChanged;
        private void changed(String name)
        {
            if (PropertyChanged != null) PropertyChanged(this, new PropertyChangedEventArgs(name));
        }

        uint _value = 0;
        uint _min = 0;
        uint _max = 0;
        uint _pendding = uint.MaxValue;

        uint _component = 0;

        MonitorSetter _setter = null;
        MonitorGetter _getter = null;
        MonitorComponentSetter _componentSetter = null;
        MonitorComponentGetter _componentGetter = null;


        public MonitorLevel(MonitorGetter getter, MonitorSetter setter)
        {
            _setter = setter;
            _getter = getter;

            getValueThread();
        }
        public MonitorLevel(MonitorComponentGetter getter, MonitorComponentSetter setter, uint component)
        {
            _component = component;
            _componentSetter = setter;
            _componentGetter = getter;

            getValueThread();
        }

        private void getValue()
        {
            uint min = 0;
            uint max = 0;
            uint value = 0;

            bool success = false;

            //if (_screen != null)
            {
                if (_getter != null)
                {
                    for (int i = 0; i < 100 && !success; i++)
                    {
                        success = _getter(ref min, ref value, ref max);
                    }

                }
                else if (_componentGetter != null)
                {
                    for (int i = 0; i < 100 && !success; i++)
                    {
                        success = _componentGetter(_component, ref min, ref value, ref max);
                    }
                }
            }

            Interlocked.Exchange(ref _locked, 0);

            if (!success) return;

            if (min != _min) { _min = min; changed("Min"); }
            if (max != _max) { _max = max; changed("Max"); }
            if (value != _value) { _value = value; changed("Value"); changed("ValueAsync"); }
        }
        private void getValueThread()
        {
            var s = new SpinWait();

            while (true)
            {
                // If CompareExchange equals 0, we won the race.
                if (Interlocked.CompareExchange(ref _locked, 1, 0) == 0)
                {
                    Thread thread = new Thread(
                    new ThreadStart(
                      delegate () { getValue(); }
                      ));

                    thread.Start();

                    break; // exit the while loop
                }

                s.SpinOnce(); // lost the race. Spin and try again.
            }

        }

        Thread _threadSetter;
        private void setValueThread()
        {
            if (_threadSetter == null || !_threadSetter.IsAlive)
            {
                _threadSetter = new Thread(
                new ThreadStart(
                  delegate ()
                  {
                      uint pendding = _pendding;

                      if (pendding != uint.MaxValue)
                      {
                          if (_setter != null)
                              _setter(pendding);
                          else if (_componentSetter != null)
                              _componentSetter(_component, pendding);

                          getValue();
                      }

                      if (_pendding == pendding)
                          _pendding = uint.MaxValue;
                      else
                          setValueThread();
                  }
                  ));

                _threadSetter.Start();
            }
        }
        public uint ValueAsync
        {
            get { return _value; }
            set
            {
                if (_pendding != value)
                {
                    _pendding = value;
                    setValueThread();
                }
            }
        }
        public uint Value
        {
            get
            {
                while (_pendding < uint.MaxValue) ;
                return _value;
            }
            set
            {
                if (_pendding != value)
                {
                    _pendding = value;
                    setValueThread();
                    while (_pendding < uint.MaxValue) ;
                }
            }
        }
        public uint Min => _min;
        public uint Max => _max;
    }
}
