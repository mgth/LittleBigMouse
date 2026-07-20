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
        : base(new CommandWorker(), VcpGetter, VcpSetter, component)
    {
        if(!Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");
    }
}

public interface IWorkProvider
{
   void DoWork();
}

public class MonitorLevel(
   CommandWorker worker,
   VcpGetter getter, VcpSetter setter,
   VcpComponent component = VcpComponent.None)
   : ReactiveObject, IWorkProvider, IDisposable
{
    int _queued;
    int _disposed;

    public VcpComponent Component { get; } = component;

    public MonitorLevel Start()
    {
        RequestWork();
        return this;
    }

    void RequestWork()
    {
        if (Volatile.Read(ref _disposed) != 0) return;
        if (Interlocked.CompareExchange(ref _queued, 1, 0) == 0)
            worker.Enqueue(this);
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

    void IWorkProvider.DoWork()
    {
        Interlocked.Exchange(ref _queued, 0);
        if (Volatile.Read(ref _disposed) != 0) return;

        // First get current value
        getter(Component).Switch((v =>
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
                    if (setter(Value, Component))
                    {
                        _retryWrite = MAX_RETRY;
                        RequestWork();
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
                        RequestWork();
                    }
                    return;
                }

                // if not moving, set local value to remote one
                this.RaiseAndSetIfChanged(ref _value, v.value, nameof(Value));
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
                    RequestWork();
                }
            });
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
            RequestWork();
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

    public void Dispose()
    {
        Interlocked.Exchange(ref _disposed, 1);
        GC.SuppressFinalize(this);
    }

}
