using Avalonia.Controls;
using HLab.Mvvm.Annotations;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Patterns;

/// <summary>
/// Logique d'interaction pour TestPatternButtonView.xaml
/// </summary>
public partial class TestPatternButtonView : UserControl, IView<TestPatternButtonViewModel>
{
    public TestPatternButtonView()
    {
        InitializeComponent();
    }
}