using Avalonia.Controls;
using Avalonia.Interactivity;

namespace LittleBigMouse.Ui.Avalonia.Updater;

/// <summary>
/// Logique d'interaction pour ApplicationUpdate.xaml
/// </summary>
public partial class ApplicationUpdaterView : Window
{
    public ApplicationUpdaterView()
    {
        InitializeComponent();
    }

    void ButtonBase_OnClick(object sender, RoutedEventArgs e)
    {
        Close();
    }
}