using System;
using ReactiveUI;
using System.Windows.Input;
using Avalonia.Controls;

namespace LittleBigMouse.Ui.Avalonia.Main;

public class UiCommandDesign : UiCommand
{
    public UiCommandDesign():base("Design")
    {
        if(!Design.IsDesignMode) throw new NotSupportedException("Only for design mode");

        IconPath = "Icons/Settings";
        ToolTipText = "Settings";
    }
}                                                                       

public class UiCommand : ReactiveObject
{
    string _iconPath = "";
    string _toolTipText = "";
    ICommand _command = null;
    bool _isActive = false;

    public UiCommand(string id)
    {
        Id = id;
    }

    public string Id { get; }

    public bool IsActive
    {
        get => _isActive;
        set => this.RaiseAndSetIfChanged(ref _isActive, value);
    }

    public string IconPath
    {
        get => _iconPath;
        set => this.RaiseAndSetIfChanged(ref _iconPath, value);
    }

    public string ToolTipText
    {
        get => _toolTipText;
        set => this.RaiseAndSetIfChanged(ref _toolTipText, value);
    }

    public ICommand Command 
    {
        get => _command;
        set => this.RaiseAndSetIfChanged(ref _command, value);
    }
}