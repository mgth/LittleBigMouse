#nullable enable
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using System.Reactive.Linq;
using System.Runtime.Serialization;
using System.Text.Json.Serialization;
using System.Threading;
using DynamicData;
using DynamicData.Binding;
using HLab.Base.ReactiveUI;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
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

      _physicalMonitorsCache.Connect()
          .StartWithEmpty()
          .Bind(out _physicalMonitors)
          .Subscribe().DisposeWith(this);

      _physicalSourcesCache.Connect()
          .Sort(SortExpressionComparer<PhysicalSource>.Ascending(t => t.DeviceId))
          .Bind(out _physicalSources)
          .Subscribe().DisposeWith(this);

      _physicalMonitorsCache.Connect()
          .AutoRefresh(e => e.DepthProjection.OutsideBounds)
          .ToCollection()
          .Do(ParsePhysicalMonitors)
          .Subscribe().DisposeWith(this);

      _physicalSourcesCache.Connect()
          .AutoRefresh(e => e.Source.EffectiveDpi.X)
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
             if (e.All(m => m.Saved)) return;
             Saved = false;
          })
          .Subscribe().DisposeWith(this);

      //if any physical source unsaved set layout not saved
      _physicalSourcesCache.Connect()
          .AutoRefresh(e => e.Saved)
          .ToCollection()
          .Do(e =>
          {
             if (e.All(m => m.Saved)) return;
             Saved = false;
          })
          .Subscribe().DisposeWith(this);

      this.WhenAnyValue(e => e.Options.Saved).Where(e => !e).Do(e => Saved = false).Subscribe().DisposeWith(this);
   }

   public ILayoutOptions Options { get; }


   /// <summary>
   /// DPI awareness of the UI process. Process-scoped, supplied by the display snapshot
   /// (see <see cref="DpiAwarenessKind"/>); the DIP-ratio bindings observe it, so it is a
   /// reactive property set once by LayoutFactory before the sources are built.
   /// </summary>
   public DpiAwarenessKind DpiAwareness
   {
      get;
      set => this.RaiseAndSetIfChanged(ref field, value);
   } = DpiAwarenessKind.PerMonitorAware;

   [DataMember]
   public string Id
   {
      get;
      set => this.RaiseAndSetIfChanged(ref field, value);
   } = "";

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
      if (result == null) throw new Exception("GetOrAddPhysicalMonitorModel failed");
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
      get;
      set => this.RaiseAndSetIfChanged(ref field, value);
   }


   public string LayoutPath(bool create) => Options.GetConfigPath(Id, create);

   /// <summary>
   /// PhysicalMonitor on witch primary source get displayed.
   /// </summary>
   [JsonIgnore]
   public PhysicalMonitor? PrimaryMonitor
   {
      get;
      private set => this.RaiseAndSetIfChanged(ref field, value);
   }

   /// <summary>
   /// Source for primary display (always located at 0,0).
   /// </summary>
   [DataMember]
   public DisplaySource? PrimarySource { get; private set => this.RaiseAndSetIfChanged(ref field, value); }

   /// <summary>
   /// Mm Bounds of overall screens without borders
   /// </summary>
   [DataMember]
   public Rect PhysicalBounds { get; set => this.RaiseAndSetIfChanged(ref field, value); }

   public void UpdatePhysicalMonitors() => ParsePhysicalMonitors(PhysicalMonitors);

   void ParsePhysicalMonitors(IEnumerable<PhysicalMonitor> monitors)
   {
      Options.MinimalMaxTravelDistance = Math.Ceiling(this.GetMinimalMaxTravelDistance());

      using var it = monitors.GetEnumerator();
      if(!it.MoveNext()) return;
      var physicalBounds = it.Current.DepthProjection.OutsideBounds;
      while (it.MoveNext())
      {
         physicalBounds = physicalBounds.Union(it.Current.DepthProjection.OutsideBounds);
      }

      PhysicalBounds = physicalBounds;
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
   public Rect ConfigLocation { get; set => SetUnsavedValue(ref field, value); }

   readonly Lock _compactLock = new();

   public void Compact()
   {
      if (Options.AllowDiscontinuity) return;
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

         monitor.PlaceAuto(done, Options.AllowDiscontinuity, Options.AllowOverlaps);
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
   public double MaxEffectiveDpiX { get; private set => this.RaiseAndSetIfChanged(ref field, value); }

   /// <summary>
   /// Maximum effective Vertical DPI of all screens
   /// </summary>
   [DataMember]
   public double MaxEffectiveDpiY { get; private set => this.RaiseAndSetIfChanged(ref field, value); }

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

   public static MonitorsLayout MonitorsLayoutDesign
   {
      get
      {
         // if(!Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");
         return new MonitorsLayout(null);
      }
   }

   /// <summary>
   /// A new layout is built and the previous one disposed on every display change:
   /// deterministically tear down the children so no subscription outlives its
   /// generation (issue #412).
   /// </summary>
   public override void OnDispose()
   {
      _physicalMonitorsCache.Dispose();
      _physicalSourcesCache.Dispose();
      _physicalMonitorModelsCache.Dispose();

      foreach (var monitor in _physicalMonitors) monitor.Dispose();
      foreach (var source in _physicalSources) source.Dispose();
   }
}
