using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using NotifyChange;
using WinAPI_Dxva2;

namespace LbmScreenConfig
{
    public static class VcpExpendMonitor
    {
        private static readonly Dictionary<Monitor, MonitorVcp> AllVcp = new Dictionary<Monitor, MonitorVcp>();
        public static MonitorVcp Vcp(this Monitor monitor)
        {
            if (AllVcp.ContainsKey(monitor)) return AllVcp[monitor];

            MonitorVcp vcp = new MonitorVcp(monitor);
            AllVcp.Add(monitor, vcp);
            return vcp;
        }
    }
    public enum Component { Red, Green, Blue, Brightness, Contrast }
    public class MonitorVcp : Notifier
    {
        public Monitor Monitor { get; }

        internal MonitorVcp(Monitor monitor)
        {
            Monitor = monitor;
            Brightness = new MonitorLevel(GetBrightness, SetBrightness);
            Contrast = new MonitorLevel(GetContrast, SetContrast);

            Gain = new MonitorRgbLevel(GetGain, SetGain);
            Drive = new MonitorRgbLevel(GetDrive, SetDrive);
        }

        public bool AlternatePower => Monitor.ManufacturerCode == "DEL";

        private bool _power = true;
        public bool Power
        {
            get { return _power; }
            set
            {
                if (!SetProperty(ref _power, value)) return;

                if (value)
                {
                    if(AlternatePower)
                        Dxva2.SetVCPFeature(Monitor.HPhysical, 0xE1, 0);
                    else
                        Dxva2.SetVCPFeature(Monitor.HPhysical, 0xD6, 1);

                }
                else
                {
                    if(AlternatePower)
                        Dxva2.SetVCPFeature(Monitor.HPhysical, 0xE1, 1);
                    else
                        Dxva2.SetVCPFeature(Monitor.HPhysical, 0xD6, 4);
                }
            }
        }


        public static DependencyProperty BrightnessProperty;
        public MonitorLevel Brightness { get; }
        public MonitorLevel Contrast { get; }

        public MonitorRgbLevel Gain { get; }
        public MonitorRgbLevel Drive { get; }

        private bool GetBrightness(ref uint min, ref uint value, ref uint max, uint component = 0)
        {
            return Dxva2.GetMonitorBrightness(Monitor.HPhysical, ref min, ref value, ref max);
        }
        private bool SetBrightness(uint value, uint component = 0)
        {
            return Dxva2.SetMonitorBrightness(Monitor.HPhysical, value);
        }
        private bool GetContrast(ref uint min, ref uint value, ref uint max, uint component = 0)
        {
            return Dxva2.GetMonitorContrast(Monitor.HPhysical, ref min, ref value, ref max);
        }
        private bool SetContrast(uint value, uint component = 0)
        {
            return Dxva2.SetMonitorContrast(Monitor.HPhysical, value);
        }
        private bool GetGain(ref uint min, ref uint value, ref uint max, uint component)
        {
            return Dxva2.GetMonitorRedGreenOrBlueGain(Monitor.HPhysical, component, ref min, ref value, ref max);
        }
        private bool SetGain(uint value, uint component)
        {
            return Dxva2.SetMonitorRedGreenOrBlueGain(Monitor.HPhysical, component, value);
        }
        private bool GetDrive(ref uint min, ref uint value, ref uint max, uint component)
        {
            return Dxva2.GetMonitorRedGreenOrBlueDrive(Monitor.HPhysical, component, ref min, ref value, ref max);
        }
        private bool SetDrive(uint component, uint value)
        {
            return Dxva2.SetMonitorRedGreenOrBlueDrive(Monitor.HPhysical, component, value);
        }
    }

    public delegate bool VcpGetter(ref uint min, ref uint value, ref uint max, uint component = 0);
    public delegate bool VcpSetter(uint value, uint component = 0);

    public class MonitorRgbLevel : Notifier
    {
        private readonly MonitorLevel[] _values = new MonitorLevel[3];

        public MonitorRgbLevel(VcpGetter getter, VcpSetter setter)
        {
            for (uint i = 0; i < 3; i++)
                _values[i] = new MonitorLevel(getter, setter, i);

        }
        public MonitorLevel Channel(uint channel) { return _values[channel]; }

        public MonitorLevel Red => Channel(0);
        public MonitorLevel Green => Channel(1);
        public MonitorLevel Blue => Channel(2);
    }

    public class MonitorLevel : Notifier
    {
        private uint _value = 0;
        private uint _min = 0;
        private uint _max = 0;

        private readonly uint _component = 0;

        private readonly VcpSetter _componentSetter = null;
        private readonly VcpGetter _componentGetter = null;

        public MonitorLevel(VcpGetter getter, VcpSetter setter, uint component = 0)
        {
            _component = component;
            _componentSetter = setter;
            _componentGetter = getter;

            _threadSetter = new LossyThread( );

            _threadSetter.Add(GetValue);
        }

        private static readonly object LockDdcCi = new object();

        public void SetToMax() { CheckedValue = Max; }

        public void SetToMin() { CheckedValue = Min; }

        private void GetValue()
        {
            if (_componentGetter == null) return;

            uint min = 0;
            uint max = 0;
            uint value = 0;

            bool result = false;
            int tries = 10;

            lock (LockDdcCi) 
                while(!result && tries-- > 0)
                    result = _componentGetter.Invoke(ref min, ref value, ref max, _component);

            if (result)
            {
                Min = min;
                Max = max;
                SetProperty(ref _value, value, "Value");               
            }
        }

        private readonly object _lockValue = new object();
        private readonly LossyThread _threadSetter;
        private uint _valueAsync;

        public uint Value
        {
            get { return _value; }
            set
            {
                //lock(_lockValue)
                if (SetProperty(ref _value, value))
                {
                        lock(LockDdcCi) _componentSetter?.Invoke(value, _component);
                        //SetProperty(ref _valueAsync, value, "ValueAsync"); 
                }
            }
        }

        [DependsOn("Value")]
        public uint CheckedValue
        {
            get { return _value;}
            set
            {
                int tries = 10;
                while (_value != value && tries-- > 0)
                {
                    Value = value;
                    GetValue();
                }
            }
        }

        [DependsOn("Value")]
        public uint ValueAsync
        {
            get { return _valueAsync; }
            set
            {
                //lock (_lockValue) //causes deadlock
                {
                    SetProperty(ref _valueAsync, value);
                    _threadSetter.Add(() =>
                    {
                        CheckedValue = value;
                    });
                }
            }
        }
        public uint Min
        {
            get { return _min; }
            private set { SetProperty(ref _min, value); }
        }

        public uint Max
        {
            get { return _max; }
            private set { SetProperty(ref _max, value); }
        }
    }
}
