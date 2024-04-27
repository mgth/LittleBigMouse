using Avalonia.Controls;
using HLab.Mvvm.Annotations;

namespace LittleBigMouse.Ui.Avalonia.Options;

public partial class LayoutOptions : UserControl, IView<LbmOptionsViewModel>
{
    public LayoutOptions()
    {
        InitializeComponent();
    }
}