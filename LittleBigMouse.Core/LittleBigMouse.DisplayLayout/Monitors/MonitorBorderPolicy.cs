using System;
using System.ComponentModel;
using System.Reactive;
using System.Reactive.Disposables;
using System.Reactive.Linq;
using System.Reactive.Subjects;
using LittleBigMouse.DisplayLayout.Dimensions;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Monitors;

/// <summary>
/// Decides which bezel borders one monitor's geometry is built from, and keeps that decision
/// consistent while the "Border values" option, the shared model and the user all move.
/// <para>
/// Three rules live here, and nothing else does:
/// <list type="number">
/// <item>which size the geometry roots at — the shared per-model one, or the same size with this
/// monitor's own borders substituted (<see cref="EffectiveSize"/>);</item>
/// <item>the mirror that keeps <see cref="Borders"/> following the model until the monitor owns
/// its values, so the first switch to "PerMonitor" starts from what the user currently sees;</item>
/// <item>the detection of that ownership (<see cref="Customized"/>) — a user edit in "PerMonitor"
/// mode, as opposed to a mirror write, which is never one.</item>
/// </list>
/// </para>
/// <para>
/// It owns no persistence and no dirty flag: it reports edits through <see cref="BordersChanged"/>
/// and lets the monitor decide what "unsaved" means. That is the whole point of the split — these
/// rules are testable against a bare size and an options object, with no monitor, layout or source
/// in sight.
/// </para>
/// </summary>
public sealed class MonitorBorderPolicy : ReactiveObject, IDisposable
{
    /// <summary>Bezel borders are shared by every monitor of the same make/model.</summary>
    public const string PerModel = "PerModel";

    /// <summary>Each physical monitor keeps its own bezel borders.</summary>
    public const string PerMonitor = "PerMonitor";

    readonly CompositeDisposable _disposables = new();
    readonly Subject<Unit> _bordersChanged = new();

    // True while the mirror below copies model borders into Borders, so the
    // customization detector can tell mirror writes from user edits.
    bool _mirroringModelBorders;

    public MonitorBorderPolicy(DisplaySizeInMm modelSize, ILayoutOptions options)
    {
        // Per-monitor border source, seeded from the model so "PerMonitor" starts matching "PerModel".
        Borders = new DisplayBorders
        {
            Left = modelSize.LeftBorder,
            Top = modelSize.TopBorder,
            Right = modelSize.RightBorder,
            Bottom = modelSize.BottomBorder,
        };

        var borderOverride = new DisplayBorderOverride(modelSize, Borders);

        // Observe BorderValues directly via ILayoutOptions.PropertyChanged — WhenAnyValue through
        // IMonitorsLayout (which declares no INotifyPropertyChanged) loses the innermost subscription.
        var borderValuesObs = Observable
            .FromEventPattern<PropertyChangedEventHandler, PropertyChangedEventArgs>(
                h => options.PropertyChanged += h,
                h => options.PropertyChanged -= h)
            .Where(e => e.EventArgs.PropertyName == nameof(ILayoutOptions.BorderValues))
            .Select(_ => options.BorderValues)
            .StartWith(options.BorderValues)
            .DistinctUntilChanged();

        IMutableDisplaySize EffectiveSizeFor(string mode) =>
            mode == PerMonitor ? borderOverride : modelSize;

        // Shared, replayed stream — Replay(1) ensures every geometry chain receives the current
        // value when it subscribes, without missing the StartWith emission that already fired for
        // the first subscriber. Consumers subscribe directly rather than through a
        // WhenAnyValue(e => e.EffectivePhysicalSize) OAPH-PropertyChanged round-trip, which breaks
        // in Avalonia's synchronous reentrancy model (mode-change event → OAPH fires → inner
        // PropertyChanged → WhenAnyValue chain stops).
        EffectiveSize = borderValuesObs
            .Select(EffectiveSizeFor)
            .Replay(1)
            .RefCount();

        // Keep the per-monitor borders following the shared model values until this
        // monitor owns some (loaded from the store, or edited in PerMonitor mode):
        // the first switch to PerMonitor must start from the monitor's CURRENT model
        // borders, not from a seed frozen at an earlier launch. Once customized the
        // mirror stops for good and the stored values win.
        _disposables.Add(modelSize.WhenAnyValue(
                e => e.LeftBorder,
                e => e.TopBorder,
                e => e.RightBorder,
                e => e.BottomBorder)
            .Where(_ => !Customized)
            .Subscribe(_ =>
            {
                // Flag the copy so the customization detector below ignores it: a
                // mirror write is never a user edit. Without this, the first model
                // border applied while the mode is already PerMonitor (e.g. during
                // Load) marks the monitor customized and cuts the mirror mid-copy.
                _mirroringModelBorders = true;
                try
                {
                    Borders.Left = modelSize.LeftBorder;
                    Borders.Top = modelSize.TopBorder;
                    Borders.Right = modelSize.RightBorder;
                    Borders.Bottom = modelSize.BottomBorder;
                }
                finally
                {
                    _mirroringModelBorders = false;
                }
            }));

        // Skip(1) drops the initial combined emission at subscription time.
        // A change landing while in PerMonitor mode is a user edit of this monitor's
        // own borders: from then on the monitor keeps them (mirror above stops).
        _disposables.Add(Borders.WhenAnyValue(
                e => e.Left,
                e => e.Top,
                e => e.Right,
                e => e.Bottom,
                (l, t, r, b) => (l, t, r, b))
            .Skip(1)
            .Subscribe(_ =>
            {
                if (!_mirroringModelBorders && options.BorderValues == PerMonitor)
                    Customized = true;

                _bordersChanged.OnNext(Unit.Default);
            }));
    }

    /// <summary>
    /// This monitor's own bezel borders, used to build the geometry only when "Border values" is
    /// "PerMonitor". Seeded from the model at construction; loaded from / saved to the monitor's
    /// own store entry. In "PerModel" mode the geometry ignores these and uses the shared model
    /// borders — but the mirror keeps them up to date all the same.
    /// </summary>
    public DisplayBorders Borders { get; }

    /// <summary>
    /// The size the geometry must be built from, re-emitted whenever the "Border values" option
    /// switches the monitor between the shared per-model size and its own border override.
    /// </summary>
    public IObservable<IMutableDisplaySize> EffectiveSize { get; }

    /// <summary>
    /// Fires on every change to <see cref="Borders"/> past the seeding, mirror writes included:
    /// <see cref="DisplayBorders"/> is not <c>ISavable</c>, so the owner cannot track it with
    /// <c>UnsavedOn</c> and needs this to mark itself dirty.
    /// </summary>
    public IObservable<Unit> BordersChanged => _bordersChanged;

    /// <summary>
    /// True once this monitor owns its bezel borders (persisted values were loaded, or a border
    /// was edited in "PerMonitor" mode). Until then <see cref="Borders"/> mirrors the shared model
    /// values, and nothing per-monitor is persisted.
    /// </summary>
    public bool Customized
    {
        get;
        set => this.RaiseAndSetIfChanged(ref field, value);
    }

    public void Dispose()
    {
        _disposables.Dispose();
        _bordersChanged.Dispose();
    }
}
