using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Reactive.Linq;
using System.Runtime.Serialization;
using System.Windows.Input;
using DynamicData;
using DynamicData.Binding;
using HLab.Base.Avalonia.Extensions;
using HLab.Base.ReactiveUI;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Localization;
using Avalonia.Threading;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Plugins.Persistence;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Zoning;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Options;

public class LbmOptionsViewModel : ViewModel<ILayoutOptions>
{
    public LbmOptionsViewModel(
        IProcessesCollector collector,
        IMainService mainService,
        AgentClient agent)
    {
        LanguageList =
        [
            LocalizedItem(LbmLocalizer.AutoLanguage, "Options.Language.Auto"),
            LocalizedItem(LbmLocalizer.DefaultLanguage, "Options.Language.English"),
            LocalizedItem(LbmLocalizer.SimplifiedChinese, "Options.Language.ChineseSimplified"),
            LocalizedItem(LbmLocalizer.German, "Options.Language.German"),
            LocalizedItem(LbmLocalizer.French, "Options.Language.French"),
            LocalizedItem(LbmLocalizer.Spanish, "Options.Language.Spanish"),
            LocalizedItem(LbmLocalizer.BrazilianPortuguese, "Options.Language.PortugueseBrazil"),
            LocalizedItem(LbmLocalizer.Italian, "Options.Language.Italian"),
            LocalizedItem(LbmLocalizer.Polish, "Options.Language.Polish"),
            LocalizedItem(LbmLocalizer.Japanese, "Options.Language.Japanese"),
            LocalizedItem(LbmLocalizer.Korean, "Options.Language.Korean")
        ];

        AlgorithmList =
        [
            LocalizedItem("Strait", "Options.Algorithm.Strait.Caption", "Options.Algorithm.Strait.Description"),
            LocalizedItem("Cross", "Options.Algorithm.Cross.Caption", "Options.Algorithm.Cross.Description")
        ];

        BorderValuesList =
        [
            LocalizedItem("PerModel", "Options.BorderValues.PerModel.Caption", "Options.BorderValues.PerModel.Description"),
            LocalizedItem("PerMonitor", "Options.BorderValues.PerMonitor.Caption", "Options.BorderValues.PerMonitor.Description")
        ];

        PriorityList =
        [
            LocalizedItem("Idle", "Options.Priority.Idle"),
            LocalizedItem("Below", "Options.Priority.Below"),
            LocalizedItem("Normal", "Options.Priority.Normal"),
            LocalizedItem("Above", "Options.Priority.Above"),
            LocalizedItem("High", "Options.Priority.High"),
            LocalizedItem("Realtime", "Options.Priority.Realtime")
        ];

        TouchModifierList =
        [
            LocalizedItem("None", "Options.Touch.Override.None"),
            new ListItem("Ctrl", "Ctrl", ""),
            new ListItem("Alt", "Alt", ""),
            new ListItem("Shift", "Shift", ""),
            new ListItem("Win", "Win", "")
        ];

        _selectedLanguage = LanguageList.Find(e => e.Id == LbmLocalizer.Current.Preference) ?? LanguageList[0];

        // A computed caption: re-read when the language changes.
        LbmLocalizer.Current
            .Observe("Options.RescueShortcut.Ownership")
            .Subscribe(_ => this.RaisePropertyChanged(nameof(ShortcutOwnership)))
            .DisposeWith(this);

        mainService.WhenAnyValue(e => e.MonitorsLayout)
            .Select(layout => layout == null
                ? Observable.Return<IReadOnlyCollection<PhysicalSource>>(Array.Empty<PhysicalSource>())
                : layout.PhysicalSources.ToObservableChangeSet().ToCollection())
            .Switch()
            .CombineLatest(this.WhenAnyValue(e => e.Model.TouchDisplayIds),
                LbmLocalizer.Current.Observe("Options.Touch.DisplayLabel"),
                (sources, ids, _) => (sources, ids))
            .ObserveOn(RxSchedulers.MainThreadScheduler)
            .Subscribe(value =>
            {
                var selected = (value.ids ?? "").Split(';').ToHashSet(StringComparer.Ordinal);
                TouchDisplays.Clear();
                foreach (var source in value.sources.Where(s =>
                    s.Source.AttachedToDesktop && s == s.Monitor.ActiveSource))
                {
                    var display = source.Source;
                    TouchDisplays.Add(new TouchDisplayChoice(display.Id,
                        LbmLocalizer.Current.Format("Options.Touch.DisplayLabel", display.SourceNumber,
                            display.DisplayName ?? source.Monitor.Model.PnpDeviceName,
                            display.InPixel.Width, display.InPixel.Height),
                        selected.Contains(display.Id), SetTouchDisplay));
                }
            }).DisposeWith(this);

        // Turning a permission OFF must fix the current layout right away, not
        // wait for the next monitor move: compact resolves the existing overlaps
        // (AllowOverlaps) or closes the existing gaps (AllowDiscontinuity).
        // Only a true→false TRANSITION compacts — loading a layout does not.
        CompactWhenTurnedOff(e => e.Model.AllowOverlaps, mainService);
        CompactWhenTurnedOff(e => e.Model.AllowDiscontinuity, mainService);

        // Editing the corridor requirement reshapes the current layout right away
        // too: compaction is what enforces the new value. Same transition rule —
        // only a CHANGE compacts, the value seen when a layout loads does not.
        // Throttled because the NumericUpDown fires on every 5mm step while the
        // spinner button is held; the throttle timer completes off the UI thread,
        // and compaction writes reactive properties the view is bound to.
        this.WhenAnyValue(e => e.Model.MinimalEdgeOverlap)
            .DistinctUntilChanged()
            .Buffer(2, 1)
            .Where(w => w.Count == 2 && !Equals(w[0], w[1]))
            .Throttle(TimeSpan.FromMilliseconds(400))
            .ObserveOn(RxSchedulers.MainThreadScheduler)
            .Subscribe(_ =>
            {
                if (mainService.MonitorsLayout is not { } layout) return;
                layout.Compact();
                layout.UpdatePhysicalMonitors();
            })
            .DisposeWith(this);

        AddExcludedProcessCommand = ReactiveCommand.Create(
            AddExcludedProcess, 
            this.WhenAnyValue(
                e => e.Model,
                
                e => e.Pattern)
            .Select(s =>
            {
                var(model, p) = s;
                if(model == null) return false;
                if(model.ExcludedList.Contains(p)) return false;
                return !string.IsNullOrWhiteSpace(p);
            }));

        RemoveExcludedProcessCommand = ReactiveCommand.Create<string?>(RemoveExcludedProcess);

        AddDefaultsCommand = ReactiveCommand.Create(AddDefaults);

        _adjustPointerAllowed = this
            .WhenAnyValue(e => e.Model.IsUnaryRatio, (bool r) => r)
            .Log(this, "_adjustPointerAllowed")
            .ToProperty(this, e => e.AdjustPointerAllowed)
            .DisposeWith(this);

        _adjustSpeedAllowed = this
            .WhenAnyValue(e => e.Model.IsUnaryRatio, (bool r) => r)
            .Log(this, "_adjustSpeedAllowed")
            .ToProperty(this, e => e.AdjustSpeedAllowed)
            .DisposeWith(this);

        _selectedAlgorithm = this.WhenAnyValue(e => e.Model.Algorithm)
            .Select(a => AlgorithmList.Find(e => e.Id == a)).ToProperty(this, nameof(SelectedAlgorithm));

        _selectedBorderValues = this.WhenAnyValue(e => e.Model.BorderValues)
            .Select(a => BorderValuesList.Find(e => e.Id == a)).ToProperty(this, nameof(SelectedBorderValues));

        _selectedPriority = this.WhenAnyValue(e => e.Model.Priority)
            .Select(a => PriorityList.Find(e => e.Id == a)).ToProperty(this, nameof(SelectedPriority));

        _selectedTouchModifier = this.WhenAnyValue(e => e.Model.TouchOverrideModifier)
            .Select(a => TouchModifierList.Find(e => e.Id == a)).ToProperty(this, nameof(SelectedTouchModifier));

        _selectedPriorityUnhooked = this.WhenAnyValue(e => e.Model.PriorityUnhooked)
            .Select(a => PriorityList.Find(e => e.Id == a)).ToProperty(this, nameof(SelectedPriorityUnhooked));

        this.WhenAnyValue(e => e.SelectedSeenProcess)
            .Subscribe(p => Pattern = p?.Caption??"")
            .DisposeWith(this);

        // The hook is what registers the rescue shortcut, so it is the only one who knows
        // whether the registration took; the agent forwards what it says. Reported rather
        // than logged: a rescue that silently does not exist is worse than none, because
        // the user only finds out at the moment they need it.
        agent.HookEventReceived += OnDaemonEvent;
        Disposer.OnDispose(() => agent.HookEventReceived -= OnDaemonEvent);

        // The hook says it once, possibly before this window existed: the agent's state
        // is what keeps it.
        ShortcutWarning = ShortcutWarningFor(agent.State);
        void OnAgentState(object? sender, AgentState state) =>
            Dispatcher.UIThread.Post(() => ShortcutWarning = ShortcutWarningFor(state));
        agent.StateChanged += OnAgentState;
        Disposer.OnDispose(() => agent.StateChanged -= OnAgentState);

        // Recorded as soon as it is set, not at the next Apply: the agent writes the
        // option and tells the hook, so the answer comes back while the user is still
        // looking at the setting. Clearing the warning makes it a fresh attempt —
        // whatever comes back is about the combination now being asked for.
        this.WhenAnyValue(e => e.Model.RescueShortcut)
            // Nothing registers it off Windows, so nothing needs telling.
            .Where(_ => RescueShortcutSupported)
            .Where(shortcut => !string.IsNullOrWhiteSpace(shortcut))
            .DistinctUntilChanged()
            .Subscribe(shortcut =>
            {
                ShortcutWarning = "";
                _ = MainService.FireAndForget(
                    () => agent.SaveOptionsAsync(
                        LayoutDtoMapper.ToGlobalOptionsDto(Model, null), null, null),
                    "Recording the rescue shortcut");
            })
            .DisposeWith(this);

        collector.SeenProcesses
            .ToObservableChangeSet()
            .Transform(p => new SeenProcessViewModel(p,p, this))
            .Bind(out _seenProcesses)
            .Subscribe()
            .DisposeWith(this);
    }

    void OnDaemonEvent(object? sender, LittleBigMouseServiceEventArgs e)
    {
        if (e.Event != LittleBigMouseEvent.ShortcutUnavailable) return;
        Dispatcher.UIThread.Post(() => ShortcutWarning = ShortcutUnavailableMessage(e.Payload));
    }

    /// <summary>What the agent's state says about the rescue shortcut; empty when it is fine.</summary>
    internal static string ShortcutWarningFor(AgentState? state, LbmLocalizer? localizer = null)
        => string.IsNullOrEmpty(state?.ShortcutUnavailable)
            ? ""
            : ShortcutUnavailableMessage(state.ShortcutUnavailable, localizer);

    /// <summary>
    /// Why the rescue is not armed — and it is not the same "why" on both platforms.
    /// Windows registers the combination itself, so the only way to fail is that
    /// something else already owns it. The desktop owns it on Linux: it can accept
    /// the shortcut and give it no key, which is not a refusal and not a collision,
    /// and which the user fixes somewhere else entirely.
    /// </summary>
    internal static string ShortcutUnavailableMessage(string shortcut, LbmLocalizer? localizer = null)
        => (localizer ?? LbmLocalizer.Current).Format(
            OperatingSystem.IsWindows()
                ? "Options.RescueShortcut.Warning"
                : "Options.RescueShortcut.Warning.Desktop",
            shortcut);

    /// <summary>
    /// Whether there is a rescue shortcut to configure. Both platforms have one now:
    /// Windows registers the combination itself, Linux asks the desktop for it
    /// through the global-shortcuts portal (#526). Anywhere else it is hidden rather
    /// than shown dead, so a build without one has nothing that looks broken.
    /// </summary>
    public bool RescueShortcutSupported => OperatingSystem.IsWindows() || OperatingSystem.IsLinux();

    /// <summary>
    /// Said on Linux whether or not the binding worked, because it is not a failure —
    /// it is who owns the shortcut. The user needs to know where it lives before they
    /// go looking for it, and where to go if they want it gone.
    /// </summary>
    public string? ShortcutOwnership => OperatingSystem.IsWindows()
        ? null
        : LbmLocalizer.Current.Get("Options.RescueShortcut.Ownership");


    public bool TouchMouseIndependentSupported => OperatingSystem.IsWindows();
    public List<ListItem> TouchModifierList { get; }

    public ListItem? SelectedTouchModifier
    {
        get => _selectedTouchModifier.Value;
        set
        {
            if (Model == null) return;
            Model.TouchOverrideModifier = value?.Id ?? "None";
        }
    }
    readonly ObservableAsPropertyHelper<ListItem?> _selectedTouchModifier;

    public ObservableCollection<TouchDisplayChoice> TouchDisplays { get; } = new();

    void SetTouchDisplay(string id, bool enabled)
    {
        if (Model == null) return;
        var ids = Model.TouchDisplayIds.Split(';', StringSplitOptions.RemoveEmptyEntries)
            .ToHashSet(StringComparer.Ordinal);
        if (enabled) ids.Add(id); else ids.Remove(id);
        Model.TouchDisplayIds = string.Join(";", ids.OrderBy(value => value, StringComparer.Ordinal));
    }


    /// <summary>Empty while the rescue shortcut is registered and working.</summary>
    public string ShortcutWarning
    {
        get => _shortcutWarning;
        private set => this.RaiseAndSetIfChanged(ref _shortcutWarning, value);
    }
    string _shortcutWarning = "";

    void CompactWhenTurnedOff(
        System.Linq.Expressions.Expression<Func<LbmOptionsViewModel, bool>> option,
        IMainService mainService)
    {
        this.WhenAnyValue(option)
            .DistinctUntilChanged()
            .Buffer(2, 1)
            .Where(w => w.Count == 2 && w[0] && !w[1])
            .Subscribe(_ =>
            {
                if (mainService.MonitorsLayout is not { } layout) return;
                layout.Compact();
                layout.UpdatePhysicalMonitors();
            })
            .DisposeWith(this);
    }

    public ICommand RemoveExcludedProcessCommand { get; }

    void RemoveExcludedProcess(string? process)
    {
        process ??= SelectedExcludedProcess;
        if (string.IsNullOrWhiteSpace(process)) return;
        if (Model == null) return;
        if (Model.ExcludedList.Contains(process)) Model.ExcludedList.Remove(process);
    }

    public ICommand AddExcludedProcessCommand { get; }
    void AddExcludedProcess()
    {
        var p = Pattern;
        if (string.IsNullOrEmpty(p)) return;
        if(Model.ExcludedList.Contains(p)) return ;

        Model.ExcludedList.Add(p);
    }

    // Top up the list with any built-in default exclusions (game launchers) not already present.
    // Restores defaults a user pruned, and surfaces newly-added ones (e.g. Xbox games, #494).
    public ICommand AddDefaultsCommand { get; }
    void AddDefaults()
    {
        if (Model == null) return;
        foreach (var entry in ExcludedProcessDefaults.All)
        {
            // Separator-insensitive: don't duplicate a Windows-style entry with its Linux twin.
            if (!ExcludedProcessDefaults.ContainsEntry(Model.ExcludedList, entry)) Model.ExcludedList.Add(entry);
        }
    }

    /// <summary>
    /// Allow speed adjustment when all displays have a pixel to dip ratio of 1
    /// </summary>
    [DataMember]
    public bool AdjustSpeedAllowed => _adjustSpeedAllowed.Value;
    readonly ObservableAsPropertyHelper<bool> _adjustSpeedAllowed;

    /// <summary>
    /// Allow pointer adjustment when all displays have a pixel to dip ratio of 1
    /// </summary>
    [DataMember]
    public bool AdjustPointerAllowed => _adjustPointerAllowed.Value;
    readonly ObservableAsPropertyHelper<bool> _adjustPointerAllowed;

    ListItem LocalizedItem(string id, string captionKey, string descriptionKey = "")
    {
        var item = new ListItem(
            id,
            LbmLocalizer.Current.Get(captionKey),
            string.IsNullOrEmpty(descriptionKey) ? "" : LbmLocalizer.Current.Get(descriptionKey));

        LbmLocalizer.Current
            .Observe(captionKey)
            .Subscribe(value => item.Caption = value)
            .DisposeWith(this);

        if (!string.IsNullOrEmpty(descriptionKey))
        {
            LbmLocalizer.Current
                .Observe(descriptionKey)
                .Subscribe(value => item.Description = value)
                .DisposeWith(this);
        }

        return item;
    }

    public List<ListItem> LanguageList { get; }

    public ListItem? SelectedLanguage
    {
        get => _selectedLanguage;
        set
        {
            if (value == null || Equals(_selectedLanguage, value)) return;
            this.RaiseAndSetIfChanged(ref _selectedLanguage, value);
            LbmLocalizer.Current.SetLanguage(value.Id, persist: true);
        }
    }
    ListItem? _selectedLanguage;

    public List<ListItem> AlgorithmList { get; }

    public List<ListItem> BorderValuesList { get; }

    public List<ListItem> PriorityList { get; }

    public ListItem? SelectedAlgorithm
    {
        get => _selectedAlgorithm.Value;
        set
        {
            if (Model == null) return;
            Model.Algorithm = value?.Id ?? "";
        }
    }
    readonly ObservableAsPropertyHelper<ListItem?> _selectedAlgorithm;

    public ListItem? SelectedBorderValues
    {
        get => _selectedBorderValues.Value;
        set
        {
            if (Model == null) return;
            Model.BorderValues = value?.Id ?? "PerModel";
        }
    }
    readonly ObservableAsPropertyHelper<ListItem?> _selectedBorderValues;

    public ListItem? SelectedPriority
    {
        get => _selectedPriority.Value;
        set
        {
            if (Model == null) return;
            Model.Priority = value?.Id ?? "";
        }
    }
    readonly ObservableAsPropertyHelper<ListItem?> _selectedPriority;
    public ListItem? SelectedPriorityUnhooked
    {
        get => _selectedPriorityUnhooked.Value;
        set
        {
            if (Model == null) return;
            Model.PriorityUnhooked = value?.Id ?? "";
        }
    }
    readonly ObservableAsPropertyHelper<ListItem?> _selectedPriorityUnhooked;

    public ReadOnlyObservableCollection<SeenProcessViewModel> SeenProcesses => _seenProcesses;
    readonly ReadOnlyObservableCollection<SeenProcessViewModel> _seenProcesses;
    public string SelectedExcludedProcess
    {
        get => _selectedExcludedProcess;
        set => this.RaiseAndSetIfChanged(ref _selectedExcludedProcess, value);
    }
    string _selectedExcludedProcess = "";
    public string Pattern
    {
        get => _pattern;
        set => this.RaiseAndSetIfChanged(ref _pattern, value);
    }
    string _pattern;

    public SeenProcessViewModel? SelectedSeenProcess
    {
        get => _selectedSeenProcess;
        set => this.RaiseAndSetIfChanged(ref _selectedSeenProcess, value);
    }
    SeenProcessViewModel? _selectedSeenProcess;

}