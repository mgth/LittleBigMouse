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
    public static class VcpExpendScreen
    {
        private static Dictionary<Screen, ScreenVcp> _allVcp = new Dictionary<Screen, ScreenVcp>();
        public static ScreenVcp Vcp(this Screen screen)
        {
            if (_allVcp.ContainsKey(screen)) return _allVcp[screen];

            ScreenVcp vcp = new ScreenVcp(screen);
            _allVcp.Add(screen, vcp);
            return vcp;
        }
    }
    public enum Component { Red, Green, Blue, Brightness, Contrast }
    public class ScreenVcp : INotifyPropertyChanged
    {

        // PropertyChanged Handling
        private readonly PropertyChangedHelper _change;
        public event PropertyChangedEventHandler PropertyChanged { add { _change.Add(this, value); } remove { _change.Remove(value); } }
        public Screen Screen { get; }

        internal ScreenVcp(Screen screen)
        {
            _change = new PropertyChangedHelper(this);

            Screen = screen;
            Brightness = new MonitorLevel(GetBrightness, SetBrightness);
            Contrast = new MonitorLevel(GetContrast, SetContrast);

            Gain = new MonitorRgbLevel(GetGain, SetGain);
            Drive = new MonitorRgbLevel(GetDrive, SetDrive);

            //            Probe = new MonitorRgbLevel(GetProbe, SetProbe);
        }

        public bool AlternatePower => Screen.ManufacturerCode == "DEL";

        private bool _power = true;
        public bool Power
        {
            get { return _power; }
            set
            {
                if (!_change.SetProperty(ref _power, value)) return;

                if (value)
                {
                    if(AlternatePower)
                        Dxva2.SetVCPFeature(Screen.HPhysical, 0xE1, 0);
                    else
                        Dxva2.SetVCPFeature(Screen.HPhysical, 0xD6, 1);

                }
                else
                {
                    if(AlternatePower)
                        Dxva2.SetVCPFeature(Screen.HPhysical, 0xE1, 1);
                    else
                        Dxva2.SetVCPFeature(Screen.HPhysical, 0xD6, 4);
                }
            }
        }


        public static DependencyProperty BrightnessProperty;
        public MonitorLevel Brightness { get; }
        public MonitorLevel Contrast { get; }

        public MonitorRgbLevel Gain { get; }
        public MonitorRgbLevel Drive { get; }
        public MonitorRgbLevel Probe { get; }

        private bool GetBrightness(ref uint min, ref uint value, ref uint max, uint component = 0)
        {
            return Dxva2.GetMonitorBrightness(Screen.HPhysical, ref min, ref value, ref max);
        }
        private bool SetBrightness(uint value, uint component = 0)
        {
            return Dxva2.SetMonitorBrightness(Screen.HPhysical, value);
        }
        private bool GetContrast(ref uint min, ref uint value, ref uint max, uint component = 0)
        {
            return Dxva2.GetMonitorContrast(Screen.HPhysical, ref min, ref value, ref max);
        }
        private bool SetContrast(uint value, uint component = 0)
        {
            return Dxva2.SetMonitorContrast(Screen.HPhysical, value);
        }
        private bool GetGain(ref uint min, ref uint value, ref uint max, uint component)
        {
            return Dxva2.GetMonitorRedGreenOrBlueGain(Screen.HPhysical, component, ref min, ref value, ref max);
        }
        private bool SetGain(uint value, uint component)
        {
            return Dxva2.SetMonitorRedGreenOrBlueGain(Screen.HPhysical, component, value);
        }
        private bool GetDrive(ref uint min, ref uint value, ref uint max, uint component)
        {
            return Dxva2.GetMonitorRedGreenOrBlueDrive(Screen.HPhysical, component, ref min, ref value, ref max);
        }
        private bool SetDrive(uint component, uint value)
        {
            return Dxva2.SetMonitorRedGreenOrBlueDrive(Screen.HPhysical, component, value);
        }
    }

    public delegate bool VcpGetter(ref uint min, ref uint value, ref uint max, uint component = 0);
    public delegate bool VcpSetter(uint value, uint component = 0);

    public class MonitorRgbLevel : DependencyObject, INotifyPropertyChanged
    {
        private readonly PropertyChangedHelper _change;
        public event PropertyChangedEventHandler PropertyChanged { add { _change.Add(this, value); } remove { _change.Remove(value); } }

        private readonly MonitorLevel[] _values = new MonitorLevel[3];

        public MonitorRgbLevel(VcpGetter getter, VcpSetter setter)
        {
            _change = new PropertyChangedHelper(this);
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

            lock (LockDdcCi)
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
                if (_threadSetter?.IsAlive ?? false) return;
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
            get { return _pendding==uint.MaxValue?_value:_pendding; }
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
                //while (_pendding != uint.MaxValue)
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
            }
        }
        public uint Min => _min;

        public uint Max => _max;
    }
}
