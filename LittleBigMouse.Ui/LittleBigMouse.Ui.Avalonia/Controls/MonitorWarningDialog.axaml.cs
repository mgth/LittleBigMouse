using System.Threading.Tasks;
using Avalonia.Controls;
using Avalonia.Interactivity;

namespace LittleBigMouse.Ui.Avalonia.Controls;

/// <summary>
/// Confirmation shown before attaching or detaching a monitor. The XAML holds the
/// detach wording; the attach variant swaps the texts in <see cref="ShowAttachAsync"/>.
/// </summary>
public partial class MonitorWarningDialog : Window
{
    bool _confirmed;

    public MonitorWarningDialog()
    {
        InitializeComponent();
    }

    void OnCancelClick(object? sender, RoutedEventArgs e) => Close(false);

    void OnConfirmClick(object? sender, RoutedEventArgs e)
    {
        _confirmed = true;
        Close(true);
    }

    public static Task<(bool Confirmed, bool DontShowAgain)> ShowDetachAsync(Window? owner)
        => new MonitorWarningDialog().ShowAsync(owner);

    public static Task<(bool Confirmed, bool DontShowAgain)> ShowAttachAsync(Window? owner)
    {
        var dialog = new MonitorWarningDialog
        {
            Title = "Attach monitor"
        };
        dialog.HeadingText.Text = "Attach this monitor to the desktop?";
        dialog.BodyText.Text = "LittleBigMouse will re-enable this monitor with its last saved position, resolution, orientation and refresh rate. If those no longer match your setup, a monitor may receive an unsupported mode or another screen may change resolution.";
        dialog.RecoveryTitle.Text = "If a screen ends up black or misconfigured:";
        dialog.ConfirmButton.Content = "Attach";

        return dialog.ShowAsync(owner);
    }

    /// <summary>
    /// Returns whether the user confirmed the action, and whether the warning
    /// should be hidden from now on (only honoured when confirmed).
    /// The app runs without an ApplicationLifetime, so the owner may be missing:
    /// in that case the dialog is still shown, centered on screen.
    /// </summary>
    async Task<(bool Confirmed, bool DontShowAgain)> ShowAsync(Window? owner)
    {
        if (owner != null)
        {
            await ShowDialog(owner);
        }
        else
        {
            var closed = new TaskCompletionSource();
            Closed += (_, _) => closed.TrySetResult();
            WindowStartupLocation = WindowStartupLocation.CenterScreen;
            Show();
            await closed.Task;
        }

        return (_confirmed, _confirmed && DontShowAgainCheckBox.IsChecked == true);
    }
}
