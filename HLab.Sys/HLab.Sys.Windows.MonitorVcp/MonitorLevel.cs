using System;
using System.Collections.Concurrent;
using System.Threading;
using Avalonia.Controls;
using HLab.Mvvm.Annotations;
using OneOf;
using ReactiveUI;

namespace HLab.Sys.Windows.MonitorVcp;

public class MonitorLevelDesign : MonitorLevel, IDesignViewModel
{
    static readonly ConcurrentDictionary<VcpComponent, uint> Components =
        new ();

    static bool  VcpSetter(uint value, VcpComponent component = VcpComponent.None)
    {
        Components.AddOrUpdate(component, value, (c,v) => 50);
        Thread.Sleep(300);
        return true;
    }

    static OneOf<(uint value, uint min, uint max), int> VcpGetter(VcpComponent component = VcpComponent.None)
    {
        var value = Components.GetOrAdd(component, c => 50);
        Thread.Sleep(300);
        return (value, 0, 100);
    }
   
    public MonitorLevelDesign(VcpComponent component = VcpComponent.None) 
        : base(new LevelParser(), VcpGetter, VcpSetter, component)
    {
        if(!Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");
    }
}

public class MonitorLevel : ReactiveObject
{
    public VcpComponent Component { get; }

    readonly VcpSetter _componentSetter;
    readonly VcpGetter _componentGetter;
    readonly LevelParser _levelParser;

    public MonitorLevel(
        LevelParser parser, 
        VcpGetter getter, 
        VcpSetter setter, 
        VcpComponent component = VcpComponent.None
        )
    {
        Component = component;
        _componentSetter = setter;
        _componentGetter = getter;

        _levelParser = parser;
    }

    public MonitorLevel Start()
    {
        _levelParser.Enqueue(this);
        return this;
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
    bool _enabled = true;

    const int MAX_RETRY = 10;
    int _retryRead = MAX_RETRY;
    int _retryWrite = MAX_RETRY;
    internal void DoWork()
    {
        // First get current value
        _componentGetter(Component).Switch((v =>
            {
                _retryRead = MAX_RETRY;
                Min = v.min;
                Max = v.max;

                // if wanted value is reached, stop
                if (v.value == Value)
                {
                    Moving = false;
                    return;
                }

                // if moving, try to set remote value
                if (Moving && Enabled)
                {
                    if (_componentSetter(Value, Component))
                    {
                        _retryWrite = MAX_RETRY;
                    }
                    else if (_retryWrite-- <= 0)
                    {
                        // if failed too many times, disable control
                        Enabled = false;
                    }
                    else
                    {
                        // if failed, wait a bit
                        Thread.Sleep(100);
                    }
                    return;
                }

                // if not moving, set local value to remote one
                Value = v.value;
                Moving = false;
            }), 
            error =>
            {
                if (_retryRead-- <= 0)
                {
                    Enabled = false;
                }
                else
                {
                    Thread.Sleep(100);
                }
            });

         _levelParser.Enqueue(this);
    }

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