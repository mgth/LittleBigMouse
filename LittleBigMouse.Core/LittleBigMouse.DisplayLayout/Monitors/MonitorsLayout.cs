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
using HLab.Base.Avalonia;
using HLab.Base.Avalonia.Extensions;
using HLab.Sys.Windows.API;
using LittleBigMouse.Zoning;
using Microsoft.Win32.TaskScheduler;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Monitors;

/// <summary>
/// 
/// </summary>
[DataContract]
public class MonitorsLayout : ReactiveModel, IMonitorsLayout
{
    public MonitorsLayout(ILayoutOptions options)
    {
        Options = options;

        DpiAwareness = WinUser.GetAwarenessFromDpiAwarenessContext(WinUser.GetThreadDpiAwarenessContext());

        _physicalMonitorModelsCache.Connect()
            .StartWithEmpty()
            .Bind(out _physicalMonitorModels)
            .Subscribe().DisposeWith(this);

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

    public PhysicalSource PhysicalSourceFromPixel(Point pixel) 
        => PhysicalSources.FirstOrDefault(source => source.Source.InPixel.Bounds.Contains(pixel));

    public PhysicalMonitor MonitorFromPhysicalPosition(Point mm) 
        => PhysicalMonitors.FirstOrDefault(screen => screen.DepthProjection.Bounds.Contains(mm));

    [JsonIgnore]
    public ReadOnlyObservableCollection<PhysicalMonitorModel> PhysicalMonitorModels => _physicalMonitorModels;
    readonly ReadOnlyObservableCollection<PhysicalMonitorModel> _physicalMonitorModels;
    readonly SourceCache<PhysicalMonitorModel, string> _physicalMonitorModelsCache = new(m => m.PnpCode);
    public void AddOrUpdatePhysicalMonitor(PhysicalMonitorModel monitor)
    {
        _physicalMonitorModelsCache.AddOrUpdate(monitor);
    }
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


    //[JsonIgnore]
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


    internal static string LayoutPath(string layoutId, bool create)
    {
        var path = Path.Combine(Environment.GetFolderPath(
            Environment.SpecialFolder.LocalApplicationData), "LittleBigMouse", layoutId);

        if (create) Directory.CreateDirectory(path);

        return path;
    }

    public string LayoutPath(bool create) => LayoutPath(Id, create);




    // TODO Move to service

    //public void MatchLayout(string id)
    //{
    //    using var rootKey = OpenRootRegKey();
    //    using var key = rootKey.OpenSubKey(@"Layouts\" + id);

    //    if (key != null)
    //    {
    //        var todo = key.GetSubKeyNames().ToList();

    //        foreach (var source in AllSources.Items)
    //        {
    //            if (todo.Contains(source.Source.IdMonitor))
    //            {
    //                AttachToDesktop(id, source.Source.IdMonitor, false);
    //                todo.Remove(source.Source.IdMonitor);
    //            }
    //            else
    //            {
    //                // TODO : Call back to monitor service to detach
    //                // MonitorDeviceHelper.DetachFromDesktop(source.Source.Device.AttachedDisplay.DeviceName, false);
    //            }
    //        }

    //        foreach (string s in todo)
    //        {
    //            AttachToDesktop(id, s, false);
    //        }

    //        MonitorDeviceHelper.ApplyDesktop();

    //    }

    //}


    // TODO :

    //public bool IsDoableLayout(string id)
    //{
    //    using var rootKey = OpenRootRegKey();
    //    using var key = rootKey.OpenSubKey(@"Layouts\" + id);

    //    if (key == null) return false;

    //    var todo = key.GetSubKeyNames().ToList();

    //    foreach (var s in todo)
    //    {
    //        var m = MonitorsService.Monitors.Items.FirstOrDefault(
    //            d => s == d.IdMonitor);

    //        if (m == null) return false;
    //    }
    //    return true;

    //}

    //TODO : move to service
    //public void AttachToDesktop(string layoutId, string monitorId, bool apply = true)
    //{
    //    //using (RegistryKey monkey = Screen.OpenMonitorRegKey(monitorId))
    //    //{
    //    //    id = monkey?.GetValue("DeviceId").ToString();
    //    //    if (id == null) return;
    //    //}
    //    var area = new Rect();
    //    var primary = false;
    //    var orientation = 0;

    //    using (var monkey = PhysicalMonitor.OpenRegKey(layoutId, monitorId))
    //    {
    //        if (monkey is not null)
    //        {
    //            area = new(
    //                double.Parse(monkey.GetValue("PixelX").ToString()),
    //                double.Parse(monkey.GetValue("PixelY").ToString()),
    //                double.Parse(monkey.GetValue("PixelWidth").ToString()),
    //                double.Parse(monkey.GetValue("PixelHeight").ToString()));

    //            primary = double.Parse(monkey.GetValue("Primary").ToString()) == 1;
    //            orientation = (int)double.Parse(monkey.GetValue("Orientation").ToString());
    //        }
    //    }

    //    var monitor = MonitorsService.Monitors.Items.FirstOrDefault(
    //        d => monitorId == d.Edid.ManufacturerCode + d.Edid.ProductCode + "_" + d.Edid.Serial);

    //    if (monitor != null)
    //        MonitorDeviceHelper.AttachToDesktop(monitor.AttachedDisplay.DeviceName, primary, area, orientation, apply);
    //}

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

    public string PrimaryMonitorId => PrimaryMonitor?.Id??"";

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

        Options.MinimalMaxTravelDistance = Math.Ceiling(GetMinimalMaxTravelDistance());

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


    public bool Schedule()
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

        td.Principal.RunLevel = TaskRunLevel.LUA;
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

    public ZonesLayout ComputeZones()
    {
        var zones = new ZonesLayout();
        foreach (var source in PhysicalSources)
        {
            if (source == source.Monitor.ActiveSource && source.Source.AttachedToDesktop)
                zones.Zones.Add(new Zone(
                    source.Monitor.BorderResistance,
                    source.Source.Id,
                    source.Monitor.Model.PnpDeviceName,
                    source.Source.InPixel.Bounds,
                    source.Monitor.DepthProjection.Bounds
                ));
        }

        var actualZones = zones.Zones.ToArray();

        if (Options.LoopX)
        {
            var shiftLeft = new Vector(-PhysicalBounds.Width, 0);
            var shiftRight = new Vector(PhysicalBounds.Width, 0);

            foreach (var zone in actualZones)
            {
                zones.Zones.Add(new (zone.BorderResistance, zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftLeft), zone));
                zones.Zones.Add(new (zone.BorderResistance, zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftRight), zone));
            }
        }

        if (Options.LoopY)
        {
            var shiftUp = new Vector(0, -PhysicalBounds.Height);
            var shiftDown = new Vector(0, PhysicalBounds.Height);

            foreach (var zone in actualZones)
            {
                zones.Zones.Add(new(zone.BorderResistance, zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftUp), zone));
                zones.Zones.Add(new(zone.BorderResistance, zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftDown), zone));
            }
        }

        zones.Init();

        zones.MaxTravelDistance = Options.MaxTravelDistance;

        zones.AdjustPointer = Options.AdjustPointer;
        zones.AdjustSpeed = Options.AdjustSpeed;

        zones.Algorithm = Options.Algorithm;
        zones.Priority = Options.Priority;
        zones.PriorityUnhooked = Options.PriorityUnhooked;

        zones.LoopX = Options.LoopX;
        zones.LoopY = Options.LoopY;

        return zones;
    }

    class MonitorDistance
    {
        public required PhysicalMonitor Source;
        public required PhysicalMonitor Target;
        public double Distance;
    }

    public double GetMinimalMaxTravelDistance()
    {
        List<MonitorDistance> distances = [];

        var primary = PrimaryMonitor;
        if (primary == null) return 0.0;

        foreach (var monitor in PhysicalMonitors)
        {
            var distance = primary.DepthProjection.Bounds.Distance(monitor.DepthProjection.Bounds).DistanceHV();
            distances.Add(new MonitorDistance
            {
                Source = primary,
                Target = monitor,
                Distance = distance,
            });
        }

        var progress = true;

        while (progress)
        {
            distances = [.. distances.OrderBy(d => d.Distance)];
            var last = distances.Last();

            var others = distances.Except([last]).ToList();

            progress = false;

            foreach (var monitorDistance in others)
            {
                //check if monitor already in chain
                var d = monitorDistance;
                while(!ReferenceEquals(d.Source, primary))
                {
                    if(ReferenceEquals(d.Target,last.Target)) break;
                    d = distances.First(e => ReferenceEquals(e.Target, d.Source));
                }
                if(ReferenceEquals(d.Target,last.Target)) continue;


                var distance = last.Target.DepthProjection.Bounds.Distance(monitorDistance.Target.DepthProjection.Bounds).DistanceHV();
                if (distance >= last.Distance) continue;
                last.Source = monitorDistance.Target;
                last.Distance = distance;
                progress = true;
            }

        }
        return distances.Max(d => d.Distance);
    }

}