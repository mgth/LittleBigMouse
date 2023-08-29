using System;
using System.Reactive.Linq;
using System.Windows.Input;
using Avalonia.Controls;
using DynamicData;
using HLab.Icons.Avalonia.Icons;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using MsBox.Avalonia;
using MsBox.Avalonia.Enums;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Main;


public class MainViewModel : ViewModel, IMainViewModel, IMainPluginsViewModel
{
    public string Title => "Little Big Mouse";

    public MainViewModel(IMonitorsLayout layout, IIconService iconService, ILocalizationService localizationService)
    {
        IconService = iconService;
        LocalizationService = localizationService;
        Layout = layout;
        CloseCommand = ReactiveCommand.Create(Close);

        //_commandsSource.Connect()
        //    .Transform(x => x)
        //    .ObserveOn(RxApp.MainThreadScheduler)
        //    .Bind(out _commands)
        //    .DisposeMany()
        //    .Subscribe();

        MaximizeCommand = ReactiveCommand.Create(() =>
            WindowState = WindowState != WindowState.Normal ? WindowState.Maximized : WindowState.Normal);
    }

    public IIconService IconService { get; }
    public ILocalizationService LocalizationService { get; }

    public IMonitorsLayout Layout { get; }

    public Type MonitorFrameViewMode
    {
        get => _monitorFrameViewMode;
        set => this.RaiseAndSetIfChanged(ref _monitorFrameViewMode, value);
    }
    Type _monitorFrameViewMode = typeof(DefaultViewMode);

    public double VerticalResizerSize => 10.0;

    public double HorizontalResizerSize => 10.0;


    public ICommand CloseCommand { get; }
    //public ICommand CloseCommand { get; } = H.Command(c => c
    //    .Action(e => e.Close())
    //);

    public ICommand MaximizeCommand { get; }

    void Close()
    {
        if (Layout?.Saved ?? true)
        {
            // TODO : exit application
            return;
        }

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
            
            //Application.Current.Shutdown();
        }

        if (result == ButtonResult.No)
        {
            //Application.Current.Shutdown();
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
        //var w = Application.Current.MainWindow;
        //if (w != null)
        WindowState = WindowState.Normal;
    }

    //readonly SourceCache<UiCommand, string> _commandsSource = new(c=>c.Id);
    //readonly ReadOnlyObservableCollection<UiCommand> _commands;
    //public ReadOnlyObservableCollection<UiCommand> Commands => _commands;

    public SourceCache<UiCommand, string> Commands { get; } = new(c => c.Id);

    public void AddButton(string id, string iconPath, string toolTipText, ICommand cmd)
    {
        var command = new UiCommand(id)
        {
            IconPath = iconPath,
            ToolTipText = toolTipText,
            Command = cmd,
        };

        Commands.AddOrUpdate(command);

        //this.RaisePropertyChanged("Commands");
    }

}