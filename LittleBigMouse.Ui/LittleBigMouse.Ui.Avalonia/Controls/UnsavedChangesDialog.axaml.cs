using System.Threading.Tasks;
using Avalonia.Controls;
using Avalonia.Interactivity;

namespace LittleBigMouse.Ui.Avalonia.Controls;

/// <summary>What the user decided on the way out.</summary>
public enum UnsavedChoice
{
    /// <summary>Stay. Nothing happens.</summary>
    Cancel,
    /// <summary>Go, leaving the layout unsaved.</summary>
    Continue,
    /// <summary>Save the layout first. Only ever offered when it has been tried.</summary>
    Save,
}

/// <summary>
/// Asked on the way out when the layout has not been saved.
/// <para>
/// <b>Save is not always on offer.</b> It appears only while live update is running,
/// because that is the only state in which the layout has actually been felt — it is
/// the one driving the mouse right now. Offering to save an untried geometry, in the
/// hurry of a dialog standing between the user and leaving, is how a layout nobody
/// wanted becomes the one that loads at the next boot.
/// </para>
/// <para>
/// The wording differs by exit: closing the window keeps the edits (the layout lives
/// on past the window, and comes back with it from the tray), quitting loses them.
/// </para>
/// </summary>
public partial class UnsavedChangesDialog : Window
{
    UnsavedChoice _choice = UnsavedChoice.Cancel;

    public UnsavedChangesDialog()
    {
        InitializeComponent();
    }

    void OnCancelClick(object? sender, RoutedEventArgs e) => Close();

    void OnContinueClick(object? sender, RoutedEventArgs e)
    {
        _choice = UnsavedChoice.Continue;
        Close();
    }

    void OnSaveClick(object? sender, RoutedEventArgs e)
    {
        _choice = UnsavedChoice.Save;
        Close();
    }

    /// <param name="leaving">
    /// True when LittleBigMouse is exiting rather than just closing its window. It
    /// decides both the warning and the wording of the third button, because it decides
    /// whether the edits survive.
    /// </param>
    /// <param name="canSave">
    /// Live update is running, so the layout has been tried and saving it is saving
    /// something known.
    /// </param>
    public static Task<UnsavedChoice> ShowAsync(Window? owner, bool leaving, bool canSave)
    {
        var dialog = new UnsavedChangesDialog();

        dialog.BodyText.Text = leaving
            ? "Your changes to the layout will be lost. The mouse engine keeps the layout you last applied."
            : "Your changes stay in LittleBigMouse and come back with the window, but they are not saved: they are lost if LittleBigMouse exits or the computer restarts. The mouse engine keeps the layout you last applied.";

        dialog.ContinueButton.Content = leaving ? "Lose changes" : "Keep unsaved";

        dialog.SaveButton.IsVisible = canSave;
        dialog.SaveNote.IsVisible = canSave;
        // Nothing to default to when there is nothing safe to recommend: leaving is a
        // choice the user has to make, not one to fall into by pressing Enter.
        if (!canSave) dialog.ContinueButton.IsDefault = true;

        return dialog.ShowChoiceAsync(owner);
    }

    async Task<UnsavedChoice> ShowChoiceAsync(Window? owner)
    {
        if (owner != null)
        {
            await ShowDialog(owner);
        }
        else
        {
            // Quitting from the tray with no window open: nothing to be modal to.
            var closed = new TaskCompletionSource();
            Closed += (_, _) => closed.TrySetResult();
            WindowStartupLocation = WindowStartupLocation.CenterScreen;
            Topmost = true;
            Show();
            await closed.Task;
        }

        return _choice;
    }
}
