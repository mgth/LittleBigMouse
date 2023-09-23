using Avalonia.Controls;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins.Avalonia;

namespace LittleBigMouse.Ui.Avalonia.Main;

public partial class UiCommandButton : UserControl, IView<UiCommand> 
{
    public UiCommandButton() {
        InitializeComponent();
    }
}