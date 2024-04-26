using Avalonia.Controls;
using HLab.Mvvm.Annotations;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Ui.Avalonia.Controls;

public partial class LayoutOptions : UserControl, IView<LbmOptionsViewModel>
{
    public LayoutOptions()
    {
        InitializeComponent();
    }
}