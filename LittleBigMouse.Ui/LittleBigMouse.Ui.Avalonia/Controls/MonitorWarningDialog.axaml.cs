using System.Threading.Tasks;
using Avalonia.Controls;
using Avalonia.Interactivity;
using LittleBigMouse.Plugins.Localization;

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
        var t = LbmLocalizer.Current;
        var dialog = new MonitorWarningDialog
        {
            Title = t.Get("Dialog.MonitorWarning.Attach.Title")
        };
        dialog.HeadingText.Text = t.Get("Dialog.MonitorWarning.Attach.Heading");
        dialog.BodyText.Text = t.Get("Dialog.MonitorWarning.Attach.Body");
        dialog.RecoveryTitle.Text = t.Get("Dialog.MonitorWarning.Attach.RecoveryTitle");
        dialog.ConfirmButton.Content = t.Get("Dialog.MonitorWarning.Attach.Confirm");

        return dialog.ShowAsync(owner);
    }

    public static Task<(bool Confirmed, bool DontShowAgain)> ShowMakePrimaryAsync(Window? owner)
    {
        var t = LbmLocalizer.Current;
        var dialog = new MonitorWarningDialog
        {
            Title = t.Get("Dialog.MonitorWarning.Primary.Title")
        };
        dialog.HeadingText.Text = t.Get("Dialog.MonitorWarning.Primary.Heading");
        dialog.BodyText.Text = t.Get("Dialog.MonitorWarning.Primary.Body");
        dialog.RecoveryTitle.Text = t.Get("Dialog.MonitorWarning.Primary.RecoveryTitle");
        dialog.Step1.Text = t.Get("Dialog.MonitorWarning.Primary.Step1");
        dialog.Step2.IsVisible = false;
        dialog.Step3.IsVisible = false;
        dialog.ConfirmButton.Content = t.Get("Dialog.MonitorWarning.Primary.Confirm");

        return dialog.ShowAsync(owner);
    }

    /// <summary>
    /// Confirmation before pushing the LBM physical layout to the system display
    /// configuration. <paramref name="offerScale"/> shows the scale checkbox (Wayland
    /// only: Windows has no supported API to change per-monitor scaling).
    /// </summary>
    public static async Task<(bool Confirmed, bool AdjustScale)> ShowApplyLayoutAsync(Window? owner, bool offerScale)
    {
        var t = LbmLocalizer.Current;
        var dialog = new MonitorWarningDialog
        {
            Title = t.Get("Dialog.MonitorWarning.ApplyLayout.Title")
        };
        dialog.HeadingText.Text = t.Get("Dialog.MonitorWarning.ApplyLayout.Heading");
        dialog.BodyText.Text = t.Get("Dialog.MonitorWarning.ApplyLayout.Body");
        dialog.RecoveryTitle.Text = t.Get("Dialog.MonitorWarning.ApplyLayout.RecoveryTitle");
        dialog.Step1.Text = t.Get("Dialog.MonitorWarning.ApplyLayout.Step1");
        dialog.Step2.IsVisible = false;
        dialog.Step3.IsVisible = false;
        dialog.DontShowAgainCheckBox.IsVisible = false;
        dialog.ScaleCheckBox.IsVisible = offerScale;
        dialog.ConfirmButton.Content = t.Get("Dialog.MonitorWarning.ApplyLayout.Confirm");

        var (confirmed, _) = await dialog.ShowAsync(owner);
        return (confirmed, confirmed && dialog.ScaleCheckBox.IsChecked == true);
    }

    /// <summary>
    /// <paramref name="action"/> is already localized: one of the
    /// <c>Dialog.MonitorWarning.Failure.Action.*</c> strings.
    /// </summary>
    public static Task ShowFailureAsync(Window? owner, string action)
    {
        var t = LbmLocalizer.Current;
        return ShowMessageAsync(
            owner,
            t.Get("Dialog.MonitorWarning.Failure.Title"),
            t.Format("Dialog.MonitorWarning.Failure.Heading", action),
            t.Get("Dialog.MonitorWarning.Failure.Body"),
            t.Get("Dialog.MonitorWarning.Failure.Step1"));
    }

    /// <summary>
    /// The layout could not be saved before being applied: the system is left as it is,
    /// since the rebuild that follows a display change would reload the saved layout and
    /// drop the one on screen.
    /// </summary>
    public static Task ShowNotAppliedAsync(Window? owner, string reason)
    {
        var t = LbmLocalizer.Current;
        return ShowMessageAsync(
            owner,
            t.Get("Dialog.MonitorWarning.NotApplied.Title"),
            t.Get("Dialog.MonitorWarning.NotApplied.Heading"),
            t.Format("Dialog.MonitorWarning.NotApplied.Body", reason),
            t.Get("Dialog.MonitorWarning.NotApplied.Step1"));
    }

    static async Task ShowMessageAsync(Window? owner, string title, string heading, string body, string step)
    {
        var t = LbmLocalizer.Current;
        var dialog = new MonitorWarningDialog
        {
            Title = title
        };
        dialog.HeadingText.Text = heading;
        dialog.BodyText.Text = body;
        dialog.RecoveryTitle.Text = t.Get("Dialog.MonitorWarning.Failure.RecoveryTitle");
        dialog.Step1.Text = step;
        dialog.Step2.IsVisible = false;
        dialog.Step3.IsVisible = false;
        dialog.DontShowAgainCheckBox.IsVisible = false;
        // The hidden CancelButton carries IsCancel: move it to the visible OK
        // button so Escape still closes the dialog.
        dialog.CancelButton.IsVisible = false;
        dialog.ConfirmButton.Content = t.Get("Dialog.MonitorWarning.Failure.Confirm");
        dialog.ConfirmButton.IsCancel = true;
        dialog.ConfirmButton.IsDefault = true;
        await dialog.ShowAsync(owner);
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
