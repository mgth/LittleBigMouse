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
using LittleBigMouse.Ui.Avalonia.Main;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Options;

public class LbmOptionsViewModel : ViewModel<ILayoutOptions>
{
    public LbmOptionsViewModel(IProcessesCollector collector)
    {
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

        RemoveExcludedProcessCommand = ReactiveCommand.Create(
            RemoveExcludedProcess,
            this.WhenAnyValue(e => e.SelectedExcludedProcess)
            .Select(s => !string.IsNullOrWhiteSpace(s)));

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

        _selectedPriority = this.WhenAnyValue(e => e.Model.Priority)
            .Select(a => PriorityList.Find(e => e.Id == a)).ToProperty(this, nameof(SelectedPriority));

        _selectedPriorityUnhooked = this.WhenAnyValue(e => e.Model.PriorityUnhooked)
            .Select(a => PriorityList.Find(e => e.Id == a)).ToProperty(this, nameof(SelectedPriorityUnhooked));

        this.WhenAnyValue(e => e.SelectedSeenProcess)
            .Subscribe(p => Pattern = p?.Caption??"")
            .DisposeWith(this);

        collector.SeenProcesses
            .ToObservableChangeSet()
            .Transform(p => new SeenProcessViewModel(p,p, this))
            .Bind(out _seenProcesses)
            .Subscribe()
            .DisposeWith(this);
    }

    public ICommand RemoveExcludedProcessCommand { get; }

    void RemoveExcludedProcess()
    {
        if (SelectedExcludedProcess is null) return;
        if (Model == null) return;
        if (Model.ExcludedList.Contains(SelectedExcludedProcess)) Model.ExcludedList.Remove(SelectedExcludedProcess);
    }

    public ICommand AddExcludedProcessCommand { get; }
    void AddExcludedProcess()
    {
        var p = Pattern;
        if (string.IsNullOrEmpty(p)) return;
        if(Model.ExcludedList.Contains(p)) return ;

        Model.ExcludedList.Add(p);
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

    public List<ListItem> AlgorithmList { get; } =
    [
        new("Strait", "Strait", "Simple and highly CPU-efficient transition."),
        new("Cross", "Corner crossing", "In direction-friendly manner, allows traversal through corners.")
    ];

    public List<ListItem> PriorityList { get; } =
    [
        new("Idle", "Idle", ""),
        new("Below", "Below", ""),
        new("Normal", "Normal", ""),
        new("Above", "Above", ""),
        new("High", "High", ""),
        new("Realtime", "Realtime", "")
    ];

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