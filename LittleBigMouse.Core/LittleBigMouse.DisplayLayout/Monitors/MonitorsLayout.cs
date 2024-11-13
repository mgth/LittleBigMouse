#nullable enable
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using System.Management;
using System.Reactive.Linq;
using System.Runtime.Serialization;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;
using Avalonia;
using Avalonia.Controls;
using DynamicData;
using DynamicData.Binding;
using HLab.Base.ReactiveUI;
using HLab.Sys.Windows.API;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using Microsoft.Win32.TaskScheduler;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Monitors;

/// <summary>
/// 
/// </summary>
[DataContract]
public class MonitorsLayout : SavableReactiveModel, IMonitorsLayout
{
    public MonitorsLayout(ILayoutOptions options)
    {
        Options = options;

        DpiAwareness = WinUser.GetAwarenessFromDpiAwarenessContext(WinUser.GetThreadDpiAwarenessContext());

        _physicalMonitorsCache.Connect()
            .StartWithEmpty()
            .Bind(out _physicalMonitors)
            .Subscribe().DisposeWith(this);

        _physicalSourcesCache.Connect()
            .Sort(SortExpressionComparer<PhysicalSource>.Ascending(t => t.DeviceId))
            .Bind(out _physicalSources)
            .Subscribe().DisposeWith(this);

        _physicalMonitorsCache.Connect()
            //.AutoRefresh(e => e.DepthProjection.Bounds)
            .ToCollection()
            .Do(ParsePhysicalMonitors)
            .Subscribe().DisposeWith(this);

        _physicalSourcesCache.Connect()
            .AutoRefresh(e => e.Source.EffectiveDpi.Y)
            .AutoRefresh(e => e.Source.EffectiveDpi.Y)
            .AutoRefresh(e => e.PixelToDipRatio)
            .ToCollection()
            .Do(ParseDisplaySources)
            .Subscribe().DisposeWith(this);

        _x0 = this
            .WhenAnyValue(e => e.PhysicalBounds.Left, left => -left)
            .Log(this, "X0").ToProperty(this, e => e.X0)
            .DisposeWith(this);

        _y0 = this
            .WhenAnyValue(e => e.PhysicalBounds.Top, top => -top)
            .Log(this, "Y0")
            .ToProperty(this, e => e.Y0)
            .DisposeWith(this);

        //if any physical monitor unsaved set layout not saved
        _physicalMonitorsCache.Connect()
            .AutoRefresh(e => e.Saved)
            .ToCollection()
            .Do(e =>
            {
                if(e.All(m => m.Saved)) return;
                Saved = false;
            })
            .Subscribe().DisposeWith(this);

        //if any physical source unsaved set layout not saved
        _physicalSourcesCache.Connect()
            .AutoRefresh(e => e.Saved)
            .ToCollection()
            .Do(e =>
            {
                if(e.All(m => m.Saved)) return;
                Saved = false;
            })
            .Subscribe().DisposeWith(this);

        this.WhenAnyValue(e => e.Options.Saved).Where(e => !e).Do(e => Saved = false).Subscribe().DisposeWith(this);
    }

    public ILayoutOptions Options { get; }

    Rect GetOutsideBounds()
    {
        using var enumerator = PhysicalMonitors.GetEnumerator();

        if (!enumerator.MoveNext()) return default;

        var r = enumerator.Current!.DepthProjection.OutsideBounds;
        while (enumerator.MoveNext())
        {
            r = r.Union(enumerator.Current.DepthProjection.OutsideBounds);
        }
        return r;
    }

    public WinDef.DpiAwareness DpiAwareness { get; }

    [DataMember]
    public string Id
    {
        get => _id;
        set => this.RaiseAndSetIfChanged(ref _id, value);
    }
    string _id = "";

    /// <summary>
    /// a list of string representing each known config in registry
    /// </summary>
    //public static IEnumerable<string> LayoutsList
    //{
    //    get
    //    {
    //        using var rootKey = MonitorsLayoutExtensions.OpenRootRegKey();
    //        using var key = rootKey?.OpenSubKey("Layouts");
    //        if (key == null) return new List<string>();
    //        return key?.GetSubKeyNames();
    //    }
    //}

    [JsonIgnore]
    readonly SourceCache<PhysicalMonitorModel, string> _physicalMonitorModelsCache = new(m => m.PnpCode);
    public PhysicalMonitorModel GetOrAddPhysicalMonitorModel(string id, Func<string, PhysicalMonitorModel> value)
    {
        PhysicalMonitorModel? result = null;
        _physicalMonitorModelsCache.Lookup(id);
        _physicalMonitorModelsCache.Edit(u =>
        {
            var lookup = u.Lookup(id);
            if (lookup.HasValue) 
            {
                result = lookup.Value;
            }
            result = value(id);
            u.AddOrUpdate(result);
        });
        if (result==null) throw new Exception("GetOrAddPhysicalMonitorModel failed");
        return result;
    }

    public ReadOnlyObservableCollection<PhysicalMonitor> PhysicalMonitors => _physicalMonitors;
    readonly ReadOnlyObservableCollection<PhysicalMonitor> _physicalMonitors;
    readonly SourceCache<PhysicalMonitor, string> _physicalMonitorsCache = new(m => m.Id);
    public void AddOrUpdatePhysicalMonitor(PhysicalMonitor monitor)
    {
        _physicalMonitorsCache.AddOrUpdate(monitor);
    }

    [JsonIgnore]
    public ReadOnlyObservableCollection<PhysicalSource> PhysicalSources => _physicalSources;
    readonly ReadOnlyObservableCollection<PhysicalSource> _physicalSources;
    readonly SourceCache<PhysicalSource, string> _physicalSourcesCache = new(s => s.Source.Id);
    public void AddOrUpdatePhysicalSource(PhysicalSource source)
    {
        _physicalSourcesCache.AddOrUpdate(source);
    }

    /// <summary>
    /// Selected physical monitor
    /// </summary>
    public PhysicalMonitor? Selected
    {
        get => _selected;
        set => this.RaiseAndSetIfChanged(ref _selected, value);
    }
    PhysicalMonitor? _selected;



    public string LayoutPath(bool create) => Options.GetConfigPath(Id, create);

    public void EnumWmi()
    {
        const string namespacePath = "\\\\.\\ROOT\\WMI\\ms_409";
        const string className = "WmiMonitorID";

        //Create ManagementClass
        var oClass = new ManagementClass(namespacePath + ":" + className);

        //Get all instances of the class and enumerate them
        foreach (var o in oClass.GetInstances().OfType<ManagementObject>())
        {
            //access a property of the Management object
            Console.WriteLine("ManufacturerName : {0}", o["ManufacturerName"]);
        }
    }

    /// <summary>
    /// PhysicalMonitor on witch primary source get displayed.
    /// </summary>
    [JsonIgnore]
    public PhysicalMonitor? PrimaryMonitor
    {
        get => _primaryMonitor;
        private set => this.RaiseAndSetIfChanged(ref _primaryMonitor, value);
    }
    PhysicalMonitor? _primaryMonitor;

    /// <summary>
    /// Source for primary display (always located at 0,0).
    /// </summary>
    [DataMember]
    public DisplaySource? PrimarySource
    {
        get => _primarySource;
        private set => this.RaiseAndSetIfChanged(ref _primarySource, value);
    }
    DisplaySource? _primarySource;

    /// <summary>
    /// Mm Bounds of overall screens without borders
    /// </summary>
    [DataMember]
    public Rect PhysicalBounds
    {
        get => _physicalBounds;
        set => this.RaiseAndSetIfChanged(ref _physicalBounds, value);
    }
    Rect _physicalBounds;

    public void UpdatePhysicalMonitors() => ParsePhysicalMonitors(PhysicalMonitors);

    void ParsePhysicalMonitors(IEnumerable<PhysicalMonitor> monitors)
    {
        Rect? physicalBounds = null;
        foreach (var m in monitors)
        {
            if (m.DepthProjection != null)
            {
                physicalBounds = physicalBounds?.Union(m.DepthProjection.OutsideBounds) ?? m.DepthProjection.OutsideBounds;
            }
        }

        Options.MinimalMaxTravelDistance = Math.Ceiling(this.GetMinimalMaxTravelDistance());

        using (DelayChangeNotifications())
        {
            PhysicalBounds = physicalBounds ?? new Rect();
        }
    }

    public double X0 => _x0.Value;

    readonly ObservableAsPropertyHelper<double> _x0;

    /// <summary>
    /// 
    /// </summary>
    public double Y0 => _y0.Value;
    readonly ObservableAsPropertyHelper<double> _y0;



    /// <summary>
    /// Store window location betwin sessions 
    /// </summary>
    [DataMember]
    public Rect ConfigLocation
    {
        get => _configLocation;
        set => SetUnsavedValue(ref _configLocation, value);
    }
    Rect _configLocation;


    readonly object _compactLock = new object();

    public void Compact()
    {
        if(Options.AllowDiscontinuity) return;
        lock (_compactLock)
        {
            ForceCompact();
        }
    }

    /// <summary>
    /// Remove all gaps between screens 
    /// </summary>
    public void ForceCompact()
    {
        // we cannot compact until primary monitor is placed
        if (PrimaryMonitor == null) return;

        // Primary monitor is always at 0,0
        List<PhysicalMonitor> done = [PrimaryMonitor];

        // Enqueue all other monitors to be placed
        var todo = this
            .PhysicalMonitors.Except(done)
            .OrderBy(monitor => monitor
                .DepthProjection.OutsideBounds.DistanceToTouch(PrimaryMonitor.DepthProjection.OutsideBounds)
                .DistanceHV()
            ).ToList();

        while (todo.Any())
        {
            var monitor = todo.First();

            monitor.PlaceAuto(done,Options.AllowDiscontinuity,Options.AllowOverlaps);
            done.Add(monitor);

            todo = [.. todo.Except([monitor]).OrderBy(s => s
                .DepthProjection.OutsideBounds.DistanceToTouch(done.Select(m => m.DepthProjection.OutsideBounds) )
                .DistanceHV()
                )];
        }
    }

    /// <summary>
    /// Maximum effective Horizontal DPI of all screens
    /// </summary>
    [DataMember]
    public double MaxEffectiveDpiX
    {
        get => _maxEffectiveDpiX;
        private set => this.RaiseAndSetIfChanged(ref _maxEffectiveDpiX, value);
    }
    double _maxEffectiveDpiX;

    /// <summary>
    /// Maximum effective Vertical DPI of all screens
    /// </summary>
    [DataMember]
    public double MaxEffectiveDpiY
    {
        get => _maxEffectiveDpiY;
        private set => this.RaiseAndSetIfChanged(ref _maxEffectiveDpiY, value);
    }
    double _maxEffectiveDpiY;

    void ParseDisplaySources(IEnumerable<PhysicalSource> sources)
    {
        double maxEffectiveDpiX = 0;
        double maxEffectiveDpiY = 0;
        var isUnaryRatio = true;
        PhysicalSource? primarySource = null;

        using (SuppressChangeNotifications())
        {
            foreach (var source in sources)
            {
                if (source.Source.Primary) { primarySource = source; }
                if (source.Source.EffectiveDpi is not null)
                {
                    maxEffectiveDpiX = Math.Max(maxEffectiveDpiX, source.Source.EffectiveDpi.X);
                    maxEffectiveDpiY = Math.Max(maxEffectiveDpiY, source.Source.EffectiveDpi.Y);
                }
                if (isUnaryRatio && source.PixelToDipRatio is { IsUnary: false }) isUnaryRatio = false;
                if (!source.Saved) Saved = false;
            }

            PrimaryMonitor = primarySource?.Monitor;
            PrimarySource = primarySource?.Source;
            MaxEffectiveDpiX = maxEffectiveDpiX;
            MaxEffectiveDpiY = maxEffectiveDpiY;
            Options.IsUnaryRatio = isUnaryRatio;
        }
    }

    string ServiceName { get; } = "LittleBigMouse_" + System.Security.Principal.WindowsIdentity.GetCurrent().Name.Replace('\\', '_');

    string ApplicationExe { get; } = AppDomain.CurrentDomain.BaseDirectory + AppDomain.CurrentDomain.FriendlyName + ".exe";

    public static MonitorsLayout MonitorsLayoutDesign
    {
        get
        {
            if(!Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");
            return new MonitorsLayout(null);
        }
    }

    public bool IsScheduled()
    {
        using var ts = new TaskService();
        return ts.RootFolder.GetTasks(new Regex(ServiceName)).Any();
    }

    public bool Schedule(bool elevated)
    {
        Unschedule();

        using var ts = new TaskService();

        //ts.RootFolder.DeleteTask(ServiceName, false);

        var td = ts.NewTask();
        td.RegistrationInfo.Description = "Multi-dpi aware monitors mouse crossover";
        td.Triggers.Add(
            //new BootTrigger());
            new LogonTrigger { UserId = System.Security.Principal.WindowsIdentity.GetCurrent().Name });

        td.Actions.Add(
            new ExecAction(ApplicationExe, "", AppDomain.CurrentDomain.BaseDirectory)
        );

        td.Principal.RunLevel = TaskRunLevel.Highest;
        td.Settings.DisallowStartIfOnBatteries = false;
        td.Settings.DisallowStartOnRemoteAppSession = true;
        td.Settings.StopIfGoingOnBatteries = false;
        td.Settings.ExecutionTimeLimit = TimeSpan.Zero;
        try
        {
            ts.RootFolder.RegisterTaskDefinition(ServiceName, td);
            return true;
        }
        catch (UnauthorizedAccessException e)
        {
        }

        td.Principal.RunLevel = elevated ? TaskRunLevel.Highest : TaskRunLevel.LUA;
        try
        {
            ts.RootFolder.RegisterTaskDefinition(ServiceName, td);
            return true;
        }
        catch (UnauthorizedAccessException e)
        {
            return false;
        }
    }

    public void Unschedule()
    {
        using var ts = new TaskService();
        try
        {
            ts.RootFolder.DeleteTask(ServiceName, false);
        }
        catch(UnauthorizedAccessException)
        {

        }
    }



}