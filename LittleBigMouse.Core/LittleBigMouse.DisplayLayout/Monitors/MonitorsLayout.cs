#nullable enable
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.IO;
using System.Linq;
using System.Management;
using System.Reactive.Linq;
using System.Runtime.Serialization;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
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
public class MonitorsLayout : ReactiveModel, IMonitorsLayout, IDisposable
{
    public MonitorsLayout()
    {
        DpiAwareness = WinUser.GetAwarenessFromDpiAwarenessContext(WinUser.GetThreadDpiAwarenessContext());

        _physicalMonitorsCache.Connect()
            // Sort Ascending on the OrderIndex property
            //.Sort(SortExpressionComparer<PhysicalMonitor>.Ascending(t => t.Id))
            //.Filter(x => x.Id.ToString().EndsWith('1'))
            .StartWithEmpty()
            .Bind(out _physicalMonitors)
            .Subscribe();

        _physicalSourcesCache.Connect()
            // Sort Ascending on the OrderIndex property
            .Sort(SortExpressionComparer<PhysicalSource>.Ascending(t => t.DeviceId))
            //.Filter(x => x.Id.ToString().EndsWith('1'))
            .Bind(out _physicalSources)
            .Subscribe();

        _physicalMonitorsCache.Connect()
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

        _adjustPointerAllowed = this
            .WhenAnyValue(e => e.IsUnaryRatio, (bool r) => r)
            .Log(this, "_adjustPointerAllowed")
            .ToProperty(this, e => e.AdjustPointerAllowed)
            .DisposeWith(this);

        _adjustSpeedAllowed = this
            .WhenAnyValue(e => e.IsUnaryRatio, (bool r) => r)
            .Log(this, "_adjustSpeedAllowed")
            .ToProperty(this, e => e.AdjustSpeedAllowed)
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
    }

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
    string _id;

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

    public PhysicalSource PhysicalSourceFromPixel(Point pixel) => PhysicalSources.FirstOrDefault(source => source.Source.InPixel.Bounds.Contains(pixel));

    public PhysicalMonitor MonitorFromPhysicalPosition(Point mm) => PhysicalMonitors.FirstOrDefault(screen => screen.DepthProjection.Bounds.Contains(mm));

    public string WallPaperPath
    {
        get => _wallPaperPath;
        set => this.RaiseAndSetIfChanged(ref _wallPaperPath, value);
    }
    string _wallPaperPath;

    public WallpaperStyle WallpaperStyle
    {
        get => _wallpaperStyle;
        set => this.RaiseAndSetIfChanged(ref _wallpaperStyle, value);
    }
    WallpaperStyle _wallpaperStyle;

    public Color BackgroundColor
    {
        get => _backgroundColor;
        set => this.RaiseAndSetIfChanged(ref _backgroundColor, value);
    }
    Color _backgroundColor = Colors.Black;

    //void MonitorsOnCollectionChanged(object sender, NotifyCollectionChangedEventArgs args)
    //{
    //    switch (args.Action)
    //    {
    //        case NotifyCollectionChangedAction.Add:
    //            if (args.NewItems != null)
    //                foreach (var device in args.NewItems.OfType<MonitorDevice>())
    //                {
    //                    var source = AllSources.Items.FirstOrDefault(s => s.Device.Equals(device));
    //                    if (source != null) continue;

    //                    var monitor = _allMonitors.FirstOrDefault(m => 
    //                        m.Model.PnpCode == device.PnpCode 
    //                        && m.ActiveSource.Device.IdPhysicalMonitor == device.IdPhysicalMonitor
    //                        );

    //                    if (monitor == null)
    //                    {
    //                        monitor = new Monitor(this, device);
    //                        source = monitor.ActiveSource;

    //                        _allMonitors.AddOrUpdate(monitor);
    //                    }
    //                    else
    //                    {
    //                        source = new MonitorSource(monitor, device);
    //                        monitor.Sources.Add(source);
    //                    }

    //                    _allSources.Add(source);
    //                }
    //            break;

    //        case NotifyCollectionChangedAction.Remove:
    //            if (args.OldItems != null)
    //                foreach (var monitor in args.OldItems.OfType<MonitorDevice>())
    //                {
    //                    var screen = AllSources.Items.FirstOrDefault(s => s.Monitor.Equals(monitor));

    //                    if (screen != null) _allSources.Remove(screen);
    //                }
    //            break;

    //        case NotifyCollectionChangedAction.Replace:
    //        case NotifyCollectionChangedAction.Move:
    //        case NotifyCollectionChangedAction.Reset:
    //            throw new NotImplementedException();
    //        default:
    //            throw new ArgumentOutOfRangeException();
    //    }


    //    Load();
    //    SetPhysicalAuto(false);
    //}

    //        [DataMember] public SourceCache<Monitor, string> AllMonitors => _allMonitors;

    [JsonIgnore]
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
    readonly SourceCache<PhysicalSource, string> _physicalSourcesCache = new(s => s.Source.IdMonitorDevice);
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


    const string ROOT_KEY = @"SOFTWARE\Mgth\LittleBigMouse";

    //internal static RegistryKey OpenLayoutRegKey(string configId, bool create)
    //{
    //    using (var key = OpenRootRegKey(create))
    //    {
    //        if (key == null) return null;
    //        return create ? key.CreateSubKey(@"configs\" + configId) : key.OpenSubKey(@"configs\" + configId);
    //    }
    //}

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






    [DataMember]
    public bool AutoUpdate
    {
        get => _autoUpdate;
        set => SetUnsavedValue(ref _autoUpdate, value);
    }
    bool _autoUpdate;

    /// <summary>
    /// PhysicalMonitor on witch primary source get displayed.
    /// </summary>
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
        var id = "";
        monitors = monitors.OrderBy(s => s.Id);
        foreach (var m in monitors)
        {
            if (!string.IsNullOrEmpty(id)) id += ".";
            id += m.Id;

            if (m.DepthProjection != null)
            {
                physicalBounds = physicalBounds?.Union(m.DepthProjection.OutsideBounds) ?? m.DepthProjection.OutsideBounds;
            }
        }

        using (DelayChangeNotifications())
        {
            Id = id;
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
    /// Mouse management enabled
    /// </summary>
    [DataMember]
    public bool Enabled
    {
        get => _enabled;
        set => SetAndRaise(ref _enabled, value);
    }
    bool _enabled;

    /// <summary>
    /// Load at startup
    /// </summary>
    [DataMember]
    public bool LoadAtStartup
    {
        get => _loadAtStartup;
        set => SetUnsavedValue(ref _loadAtStartup, value);
    }
    bool _loadAtStartup;

    /// <summary>
    /// Load at startup
    /// </summary>
    [DataMember]
    public string Priority
    {
        get => _priority;
        set => SetUnsavedValue(ref _priority, value);
    }
    string _priority;

    [DataMember]
    public bool LoopAllowed => true;

    /// <summary>
    /// Allow mouse cursor looping in X direction
    /// </summary>
    [DataMember]
    public bool LoopX
    {
        get => LoopAllowed && _loopX;
        set => SetUnsavedValue(ref _loopX, value);
    }
    bool _loopX;

    /// <summary>
    /// Allow mouse cursor looping in Y direction
    /// </summary>
    [DataMember]
    public bool LoopY
    {
        get => LoopAllowed && _loopY;
        set => SetUnsavedValue(ref _loopY, value);
    }
    bool _loopY;

    /// <summary>
    /// True if all sources have a pixel to dip ratio of 1
    /// </summary>
    [DataMember]
    public bool IsUnaryRatio
    {
        get => _isUnaryRatio;
        set => this.RaiseAndSetIfChanged(ref _isUnaryRatio, value);
    }
    bool _isUnaryRatio;


    /// <summary>
    /// Allow pointer adjustment when all displays have a pixel to dip ratio of 1
    /// </summary>
    [DataMember]
    public bool AdjustPointerAllowed => _adjustPointerAllowed.Value;
    readonly ObservableAsPropertyHelper<bool> _adjustPointerAllowed;

    /// <summary>
    /// Adjust pointer size with display pixel to dip ratio
    /// </summary>
    [DataMember]
    public bool AdjustPointer
    {
        get => AdjustPointerAllowed && _adjustPointer;
        set => SetUnsavedValue(ref _adjustPointer, value);
    }
    bool _adjustPointer;

    /// <summary>
    /// Allow speed adjustment when all displays have a pixel to dip ratio of 1
    /// </summary>
    [DataMember]
    public bool AdjustSpeedAllowed => _adjustSpeedAllowed.Value;
    readonly ObservableAsPropertyHelper<bool> _adjustSpeedAllowed;

    /// <summary>
    /// Adjust speed with display pixel to dip ratio
    /// </summary>
    [DataMember]
    public bool AdjustSpeed
    {
        get => AdjustSpeedAllowed && _adjustSpeed;
        set => SetUnsavedValue(ref _adjustSpeed, value);
    }
    bool _adjustSpeed;

    /// <summary>
    /// Experimental : Sleep monitors not containing mouse cursor after a delay 
    /// </summary>
    [DataMember]
    public bool HomeCinema
    {
        get => _homeCinema;
        set => SetUnsavedValue(ref _homeCinema, value);
    }
    bool _homeCinema;

    /// <summary>
    /// Keep window on top
    /// </summary>
    [DataMember]
    public bool Pinned
    {
        get => _pinned;
        set => SetUnsavedValue(ref _pinned, value);
    }
    bool _pinned;

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
        if(AllowDiscontinuity) return;

        ForceCompact();
    }

    /// <summary>
    /// Remove all gaps between screens 
    /// </summary>
    public void ForceCompact()
    {
        // we cannot compact until primary monitor is placed
        if (PrimaryMonitor == null) return;

        lock (_compactLock)
        {
            // Primary monitor is always at 0,0
            var done = new List<PhysicalMonitor> { PrimaryMonitor };

            // Enqueue all other monitors to be placed
            var todo = this.PhysicalMonitorsExcept(PrimaryMonitor).OrderBy(s => s.DistanceHV(PrimaryMonitor));

            while (todo.Any())
            {
                var monitor = todo.First();

                monitor.PlaceAuto(done,AllowDiscontinuity,AllowOverlaps);
                done.Add(monitor);

                todo = todo.Except([monitor]).OrderBy(s => s.DistanceHV(done));
            }
        }
    }

    /// <summary>
    /// allow monitors to overlap, may be useful for overlapped borders
    /// </summary>
    [DataMember]
    public bool AllowOverlaps
    {
        get => _allowOverlaps;
        set => SetUnsavedValue(ref _allowOverlaps, value);
    }
    bool _allowOverlaps;

    /// <summary>
    /// allow monitors to be placed with a gap between them
    /// </summary>
    [DataMember]
    public bool AllowDiscontinuity
    {
        get => _allowDiscontinuity;
        set => SetUnsavedValue(ref _allowDiscontinuity, value);
    }
    bool _allowDiscontinuity;

    /// <summary>
    /// algorithm to be used for mouse movements
    /// - Strait
    /// - CornerCrossing
    /// </summary>
    [DataMember]
    public string Algorithm 
    {
        get => _algorithm;
        set => SetUnsavedValue(ref _algorithm, value);
    }
    string _algorithm;


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
            IsUnaryRatio = isUnaryRatio;
        }
    }

    string ServiceName { get; } = "LittleBigMouse_" + System.Security.Principal.WindowsIdentity.GetCurrent().Name.Replace('\\', '_');

    string ApplicationExe { get; } = AppDomain.CurrentDomain.BaseDirectory + AppDomain.CurrentDomain.FriendlyName + ".exe";

    public static MonitorsLayout MonitorsLayoutDesign
    {
        get
        {
            if(!Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");
            return new MonitorsLayout();
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
                    source.Source.IdMonitorDevice,
                    source.Monitor.Model.PnpDeviceName,
                    source.Source.InPixel.Bounds,
                    source.Monitor.DepthProjection.Bounds
                ));
        }

        var actualZones = zones.Zones.ToArray();

        if (LoopX)
        {
            var shiftLeft = new Vector(-PhysicalBounds.Width, 0);
            var shiftRight = new Vector(PhysicalBounds.Width, 0);

            foreach (var zone in actualZones)
            {
                zones.Zones.Add(new Zone(zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftLeft), zone));
                zones.Zones.Add(new Zone(zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftRight), zone));
            }
        }

        if (LoopY)
        {
            var shiftUp = new Vector(0, -PhysicalBounds.Height);
            var shiftDown = new Vector(0, PhysicalBounds.Height);

            foreach (var zone in actualZones)
            {
                zones.Zones.Add(new Zone(zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftUp), zone));
                zones.Zones.Add(new Zone(zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftDown), zone));
            }
        }

        zones.Init();

        zones.AdjustPointer = AdjustPointer;
        zones.AdjustSpeed = AdjustSpeed;

        zones.Algorithm = Algorithm;
        zones.Priority = Priority;

        zones.LoopX = LoopX;
        zones.LoopY = LoopY;

        return zones;
    }

}