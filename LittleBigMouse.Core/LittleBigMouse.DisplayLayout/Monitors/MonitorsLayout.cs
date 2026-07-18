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
      _physicalMonitorModelsCache.Edit(u =>
      {
         var lookup = u.Lookup(id);
         if (lookup.HasValue)
         {
            // Reuse the existing model: monitors sharing a PnpCode (identical make/model) must share
            // ONE PhysicalMonitorModel, so model-level edits (physical size, borders, PnP name) apply
            // to all of them and persist consistently. Recreating here defeated the cache — two
            // identical monitors got separate instances, and one's stale Save overwrote the other's
            // registry key, silently dropping the user's change.
            result = lookup.Value;
         }
         else
         {
            result = value(id);
            u.AddOrUpdate(result);
         }
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
   /// Remove all gaps between screens.
   /// Two phases: first resolve overlaps monitor by monitor (any random start
   /// position becomes a valid spread), then pull each connected cluster of
   /// touching monitors as a RIGID group toward the primary's cluster, nearest
   /// first. Moving clusters — not individual monitors — preserves the relative
   /// arrangement: after a primary drag the whole translated group comes back
   /// as one block instead of each monitor grabbing the first free edge.
   /// </summary>
   public void ForceCompact()
   {
      // we cannot compact until primary monitor is placed
      if (PrimaryMonitor == null) return;

      var monitors = PhysicalMonitors.ToList();
      if (monitors.Count < 2) return;

      if (!Options.AllowOverlaps) ResolveOverlaps(monitors);

      // Primary monitor is always at 0,0: its cluster anchors everything.
      var clusters = BuildClusters(monitors);
      var anchored = clusters.First(c => c.Contains(PrimaryMonitor));
      var todo = clusters.Where(c => !ReferenceEquals(c, anchored)).ToList();

      while (todo.Count > 0)
      {
         var cluster = todo
             .OrderBy(c => PairDistanceToTouch(c, anchored).DistanceHV())
             .First();
         todo.Remove(cluster);

         if (!Options.AllowDiscontinuity)
         {
            var distance = PairDistanceToTouch(cluster, anchored);
            var min = distance.MinPositive();

            double dx = 0, dy = 0;
            if (min > 0 && !double.IsInfinity(min))
            {
               // Single-axis touch, same edge choice as the historical PlaceAuto.
               if (distance.Left > 0 && distance.Left <= min) dx = -distance.Left;
               else if (distance.Top > 0 && distance.Top <= min) dy = -distance.Top;
               else if (distance.Right > 0 && distance.Right <= min) dx = distance.Right;
               else if (distance.Bottom > 0 && distance.Bottom <= min) dy = distance.Bottom;
            }
            else if (double.IsInfinity(min))
            {
               // Diagonal-only reachability: close both gaps to a corner contact.
               var raw = PairDistance(cluster, anchored);
               if (raw.Left > 0) dx = -raw.Left;
               else if (raw.Right > 0) dx = raw.Right;
               if (raw.Top > 0) dy = -raw.Top;
               else if (raw.Bottom > 0) dy = raw.Bottom;
            }

            if (dx != 0 || dy != 0)
            {
               foreach (var monitor in cluster)
               {
                  var projection = monitor.DepthProjection;
                  using (projection.DelayChangeNotifications())
                  {
                     projection.X += dx;
                     projection.Y += dy;
                  }
               }
            }
         }

         anchored.AddRange(cluster);
      }
   }

   /// <summary>
   /// Push overlapping monitors apart. The primary never moves. Out of the four
   /// ways to leave an overlap, take the shortest push that does not land on yet
   /// another monitor (a blind least-penetration push can bounce between two
   /// neighbours forever); fall back to the shortest push when every direction is
   /// occupied, and iterate until stable.
   /// </summary>
   void ResolveOverlaps(List<PhysicalMonitor> monitors)
   {
      for (var pass = 0; pass < monitors.Count + 4; pass++)
      {
         var moved = false;
         foreach (var monitor in monitors)
         {
            if (ReferenceEquals(monitor, PrimaryMonitor)) continue;

            var others = monitors.Where(m => !ReferenceEquals(m, monitor)).ToList();
            var overlapped = others.FirstOrDefault(o => Overlap(monitor.DepthProjection.OutsideBounds, o.DepthProjection.OutsideBounds));
            if (overlapped == null) continue;

            var d = monitor.DepthProjection.OutsideBounds
                .Distance(overlapped.DepthProjection.OutsideBounds);

            // Right, left, below, above — as (dx, dy) displacements.
            var candidates = new[]
                {
                   (dx: -d.Left, dy: 0.0),
                   (dx: d.Right, dy: 0.0),
                   (dx: 0.0, dy: -d.Top),
                   (dx: 0.0, dy: d.Bottom)
                }
                .OrderBy(c => Math.Abs(c.dx) + Math.Abs(c.dy))
                .ToList();

            var free = candidates.Where(c =>
            {
               var bounds = monitor.DepthProjection.OutsideBounds;
               var pushed = new Rect(new Point(bounds.X + c.dx, bounds.Y + c.dy), bounds.Size);
               return !others.Any(o => Overlap(pushed, o.DepthProjection.OutsideBounds));
            }).ToList();

            var (dx, dy) = free.Count > 0 ? free[0] : candidates[0];

            var projection = monitor.DepthProjection;
            using (projection.DelayChangeNotifications())
            {
               projection.X += dx;
               projection.Y += dy;
            }
            moved = true;
         }
         if (!moved) return;
      }
   }

   static bool Overlap(Rect a, Rect b)
   {
      var d = a.Distance(b);
      return Math.Max(d.Left, d.Right) < -ContactEpsilon
          && Math.Max(d.Top, d.Bottom) < -ContactEpsilon;
   }

   // Contacts are computed values: allow rounding noise when deciding whether
   // two monitors touch (or overlap, when overlaps are permitted).
   const double ContactEpsilon = 0.5;

   static bool AreConnected(PhysicalMonitor a, PhysicalMonitor b)
   {
      var d = a.DepthProjection.OutsideBounds.Distance(b.DepthProjection.OutsideBounds);
      return Math.Max(d.Left, d.Right) <= ContactEpsilon
          && Math.Max(d.Top, d.Bottom) <= ContactEpsilon;
   }

   static List<List<PhysicalMonitor>> BuildClusters(List<PhysicalMonitor> monitors)
   {
      var clusters = new List<List<PhysicalMonitor>>();
      var remaining = new List<PhysicalMonitor>(monitors);

      while (remaining.Count > 0)
      {
         var cluster = new List<PhysicalMonitor> { remaining[0] };
         remaining.RemoveAt(0);

         var grown = true;
         while (grown)
         {
            grown = false;
            for (var i = remaining.Count - 1; i >= 0; i--)
            {
               if (!cluster.Any(m => AreConnected(m, remaining[i]))) continue;
               cluster.Add(remaining[i]);
               remaining.RemoveAt(i);
               grown = true;
            }
         }
         clusters.Add(cluster);
      }
      return clusters;
   }

   static Thickness PairDistanceToTouch(List<PhysicalMonitor> cluster, List<PhysicalMonitor> anchored)
      => cluster
          .Select(m => m.DepthProjection.OutsideBounds
              .DistanceToTouch(anchored.Select(a => a.DepthProjection.OutsideBounds)))
          .Aggregate(MonitorExtensions.Infinity, (acc, d) => acc.Min(d));

   static Thickness PairDistance(List<PhysicalMonitor> cluster, List<PhysicalMonitor> anchored)
      => cluster
          .Select(m => m.DepthProjection.OutsideBounds
              .Distance(anchored.Select(a => a.DepthProjection.OutsideBounds)))
          .Aggregate(new Thickness(double.MaxValue), (acc, d) => acc.Min(d));

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
