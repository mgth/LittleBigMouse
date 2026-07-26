using Avalonia.Controls;
using Avalonia.Interactivity;
using HLab.Mvvm.Annotations;

namespace LittleBigMouse.Ui.Avalonia.Options;

public partial class LayoutOptions : UserControl, IView<LbmOptionsViewModel>
{
    public LayoutOptions()
    {
        InitializeComponent();
    }

    void Kofi_OnClick(object sender, RoutedEventArgs e)
    {
        SupportLinks.OpenKofi();
    }
}