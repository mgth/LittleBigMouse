using System;
using System.Collections.ObjectModel;
using System.IO;
using System.Runtime.Serialization;
using HLab.Base.Avalonia;
using HLab.Base.ReactiveUI;
using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Main;


public class LbmOptions : SavableReactiveModel, ILayoutOptions
{
    public LbmOptions()
    {
        ExcludedList.CollectionChanged += (sender, args) => Saved = false;
    }

    [DataMember]
    public bool AutoUpdate
    {
        get => _autoUpdate;
        set => SetUnsavedValue(ref _autoUpdate, value);
    }
    bool _autoUpdate;

    [DataMember]
    public bool LoadAtStartup
    {
        get => _loadAtStartup;
        set => SetUnsavedValue(ref _loadAtStartup, value);
    }
    bool _loadAtStartup;

    [DataMember]
    public bool StartMinimized
    {
        get => _startMinimized;
        set =>  SetUnsavedValue(ref _startMinimized, value);
    }
    bool _startMinimized;
    
    [DataMember]
    public bool StartElevated
    {
        get => _startElevated;
        set =>  SetUnsavedValue(ref _startElevated, value);
    }
    bool _startElevated;

    [DataMember]
    public bool Elevated 
    { 
        get => _elevated; 
        set => this.SetAndRaise(ref _elevated, value); 
    }
    bool _elevated = false;

    public int DaemonPort
    { 
        get => _daemonPort; 
        set => this.SetAndRaise(ref _daemonPort, value); 
    }
    int _daemonPort = 25196;

    [DataMember]
    public string Priority 
    {
        get => _priority;
        set => SetUnsavedValue(ref _priority, value);
    }
    string _priority = "Normal";

    [DataMember]
    public string PriorityUnhooked
    {
        get => _priorityUnhooked;
        set => SetUnsavedValue(ref _priorityUnhooked, value);
    }
    string _priorityUnhooked = "Below";

    [DataMember]
    public bool Enabled
    {
        get => _enabled;
        set => this.SetAndRaise(ref _enabled, value);
    }
    bool _enabled;

    [DataMember]
    public bool LoopAllowed => true;

    [DataMember]
    public bool LoopX
    {
        get => LoopAllowed && _loopX;
        set => SetUnsavedValue(ref _loopX, value);
    }
    bool _loopX;

    [DataMember]
    public bool LoopY
    {
        get => LoopAllowed && _loopY;
        set => SetUnsavedValue(ref _loopY, value);
    }
    bool _loopY;

    [DataMember]
    public bool IsUnaryRatio
    {
        get => _isUnaryRatio;
        set => this.RaiseAndSetIfChanged(ref _isUnaryRatio, value);
    }
    bool _isUnaryRatio;

    [DataMember]
    public bool AdjustPointer
    {
        get => _adjustPointer;
        set => SetUnsavedValue(ref _adjustPointer, value);
    }
    bool _adjustPointer;

    [DataMember]
    public bool AdjustSpeed
    {
        get => _adjustSpeed;
        set => SetUnsavedValue(ref _adjustSpeed, value);
    }
    bool _adjustSpeed;

    [DataMember]
    public bool HomeCinema
    {
        get => _homeCinema;
        set => SetUnsavedValue(ref _homeCinema, value);
    }
    bool _homeCinema;

    [DataMember]
    public double MaxTravelDistance
    {
        get => _maxTravelDistance;
        set => SetUnsavedValue(ref _maxTravelDistance, value);
    }
    double _maxTravelDistance = 200.0;

    [DataMember]
    public double MinimalMaxTravelDistance
    {
        get => _minimalMaxTravelDistance;
        set => this.SetAndRaise(ref _minimalMaxTravelDistance, value);
    }
    double _minimalMaxTravelDistance = 0.0;

    [DataMember]
    public bool Pinned
    {
        get => _pinned;
        set => SetUnsavedValue(ref _pinned, value);
    }
    bool _pinned;

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
    string _algorithm = "Strait";

    public ObservableCollection<string> ExcludedList { get; } = new();

    public string GetConfigPath(string layoutId, bool create)
    {
        var path = Path.Combine(Environment.GetFolderPath(
            Environment.SpecialFolder.LocalApplicationData), "Mgth", "LittleBigMouse", layoutId);

        if (create) Directory.CreateDirectory(path);

        return path;
    }



}