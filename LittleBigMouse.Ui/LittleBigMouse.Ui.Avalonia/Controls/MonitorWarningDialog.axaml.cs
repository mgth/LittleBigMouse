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

    public static Task<(bool Confirmed, bool DontShowAgain)> ShowMakePrimaryAsync(Window? owner)
    {
        var dialog = new MonitorWarningDialog
        {
            Title = "Make primary monitor"
        };
        dialog.HeadingText.Text = "Make this monitor the primary display?";
        dialog.BodyText.Text = "Windows anchors the desktop on the primary display: every monitor position changes, open windows may move, and the taskbar goes to the new primary. LittleBigMouse will re-anchor its layout accordingly.";
        dialog.RecoveryTitle.Text = "To revert:";
        dialog.Step1.Text = "Use 'Make primary' on the previous monitor, or in Settings > System > Display select it and check 'Make this my main display'.";
        dialog.Step2.IsVisible = false;
        dialog.Step3.IsVisible = false;
        dialog.ConfirmButton.Content = "Make primary";

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
