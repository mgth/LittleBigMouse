using System;
using System.Collections.ObjectModel;
using System.Linq;
using System.Reactive.Linq;
using System.Windows.Input;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Platform;
using DynamicData;
using DynamicData.Binding;
using HLab.Base.Avalonia;
using HLab.Base.Avalonia.Extensions;
using HLab.Base.ReactiveUI;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using MsBox.Avalonia;
using MsBox.Avalonia.Enums;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Main;

public interface IProcessesCollector
{
    ObservableCollectionExtended<string> SeenProcesses { get; }
    void AddProcess(string process);
    
}

public class ProcessesCollector : ReactiveModel, IProcessesCollector
{
    public ObservableCollectionExtended<string> SeenProcesses { get; } = [];
    
    public void AddProcess(string process)
    {
        if(string.IsNullOrEmpty(process)) return;

        if (SeenProcesses.Any(e => e?.Contains(process)??false)) return;

        SeenProcesses.Add(process);
    }


}

public class MainViewModel : ViewModel, IMainViewModel, IMainPluginsViewModel
{
    public string Title => "Little Big Mouse";
    public object MainIcon { get; } = new WindowIcon(AssetLoader.Open(new Uri("avares://LittleBigMouse.Ui.Avalonia/Assets/lbm-logo.ico")));

    public MainViewModel(IIconService iconService, ILocalizationService localizationService,  ILayoutOptions options)
    {
        IconService = iconService;
        LocalizationService = localizationService;
        Options = options;
        
        CloseCommand = ReactiveCommand.Create(Close);

        _commandsCache.Connect()
            .Sort(SortExpressionComparer<IUiCommand>.Ascending(t => t.Id))
            .Bind(out _commands)
            .Subscribe().DisposeWith(this);

        MaximizeCommand = ReactiveCommand.Create(() =>
            WindowState = WindowState != WindowState.Normal ? WindowState.Maximized : WindowState.Normal);

        _presenterViewMode = this.WhenAnyValue(v => v.ViewList)
            .Select(v => v ? typeof(ListViewMode) : typeof(DefaultViewMode))
            .ToProperty(this, v => v.PresenterViewMode);
    }

    public IMainService? MainService 
    { 
        get => _mainService; 
        set => this.SetAndRaise(ref _mainService, value);
    }
    IMainService? _mainService;

    public IIconService IconService { get; }
    public ILocalizationService LocalizationService { get; }
    public ILayoutOptions Options { get; }

    public Type ContentViewMode
    {
        get => _contentViewMode;
        set => this.RaiseAndSetIfChanged(ref _contentViewMode, value);
    }
    Type _contentViewMode = typeof(DefaultViewMode);

    public Type ContentViewClass
    {
        get => _contentViewClass;
        set => this.RaiseAndSetIfChanged(ref _contentViewClass, value);
    }
    Type _contentViewClass = typeof(IDefaultViewClass);

    public object? Content
    {
        get => _content;
        set => this.RaiseAndSetIfChanged(ref _content, value);
    }
    object? _content = null;

    public Type PresenterViewMode => _presenterViewMode.Value;
    readonly ObservableAsPropertyHelper<Type> _presenterViewMode;

    public bool ViewList
    {
        get => _viewList;
        set => this.RaiseAndSetIfChanged(ref _viewList, value);
    }
    bool _viewList = false;

    public bool OptionsIsVisible
    {
        get => _optionsIsVisible;
        set => this.RaiseAndSetIfChanged(ref _optionsIsVisible, value);
    }
    bool _optionsIsVisible = false;

    public double VerticalResizerSize => 10.0;

    public double HorizontalResizerSize => 10.0;


    public ICommand CloseCommand { get; }

    public ICommand MaximizeCommand { get; }

    void Close()
    {
        //if (Layout?.Saved ?? true)
        //{
        //    // TODO : exit application
        //    return;
        //}

        var result = MessageBoxManager
            .GetMessageBoxStandard(
                "Save your changes before exiting ?",
                "Confirmation", ButtonEnum.OkCancel,
                Icon.Question, WindowStartupLocation.CenterOwner

                )
            //Todo get rid of Result
            .ShowAsync().Result;

        if (result == ButtonResult.Yes)
        {
            /* Todo avalonia*/
            //Layout.Save();
            Shutdown();
        }

        if (result == ButtonResult.No)
        {
            Shutdown();
        }
    }

    public void Shutdown()
    {
        if (Application.Current?.ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktopApp)
        {
            desktopApp.Shutdown();
        }
    }

    public WindowState WindowState
    {
        get => _windowState;
        set => this.RaiseAndSetIfChanged(ref _windowState, value);
    }
    WindowState _windowState;


    public void UnMaximize()
    {
        WindowState = WindowState.Normal;
    }

    readonly ReadOnlyObservableCollection<IUiCommand> _commands;
    public ReadOnlyObservableCollection<IUiCommand> Commands => _commands;

    SourceCache<IUiCommand, string> _commandsCache { get; } = new(c => c.Id);


    public void AddButton(IUiCommand command)
    {
        _commandsCache.AddOrUpdate(command);
    }

}