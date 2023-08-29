using System;
using Avalonia.Controls;

namespace LittleBigMouse.Ui.Avalonia.Main;

public class UiCommandDesign : UiCommand
{
    public UiCommandDesign():base("Design")
    {
        if(!Design.IsDesignMode) throw new NotSupportedException("Only for design mode");

        IconPath = "Icon/Parts/Power";
        ToolTipText = "Settings";
    }
}