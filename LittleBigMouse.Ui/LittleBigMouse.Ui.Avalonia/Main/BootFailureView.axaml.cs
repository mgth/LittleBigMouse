#nullable enable
using System;
using System.Diagnostics;
using System.IO;
using System.Threading.Tasks;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;

namespace LittleBigMouse.Ui.Avalonia.Main;

/// <summary>
/// The window a failed start puts up (see <see cref="BootFailureHandler"/>): the exception
/// as text, ready to be copied into an issue, and where the log went. Nothing else of the
/// app is on screen at that point, hence a top-level window centered on the screen with a
/// taskbar entry — it must be findable, it is the only sign the app gives.
/// </summary>
public partial class BootFailureView : Window
{
    readonly string _details;
    readonly string? _logFile;

    /// <summary>For the XAML previewer only.</summary>
    public BootFailureView() : this(new InvalidOperationException("Preview"), null) { }

    public BootFailureView(Exception error, string? logFile)
    {
        InitializeComponent();

        _details = error.ToString();
        _logFile = logFile;

        DetailsText.Text = _details;
        LogText.Text = logFile is null
            ? "This report was also written to the standard error output."
            : $"This report was also written to {logFile}.";
        OpenLogButton.IsVisible = logFile is not null;
    }

    /// <summary>Show, and complete once the user has closed it.</summary>
    public static Task ShowAsync(Exception error, string? logFile)
    {
        var view = new BootFailureView(error, logFile);
        var closed = new TaskCompletionSource();
        view.Closed += (_, _) => closed.TrySetResult();
        view.Show();
        return closed.Task;
    }

    async void OnCopyClick(object? sender, RoutedEventArgs e)
    {
        try
        {
            var clipboard = TopLevel.GetTopLevel(this)?.Clipboard;
            if (clipboard is null) return;

            var data = new DataTransfer();
            data.Add(DataTransferItem.CreateText(_details));
            await clipboard.SetDataAsync(data);
        }
        catch (Exception error)
        {
            Console.Error.WriteLine($"Copying the boot failure details failed: {error}");
        }
    }

    void OnOpenLogClick(object? sender, RoutedEventArgs e)
    {
        if (Path.GetDirectoryName(_logFile) is not { } folder) return;
        try
        {
            Process.Start(new ProcessStartInfo(folder) { UseShellExecute = true });
        }
        catch (Exception error)
        {
            Console.Error.WriteLine($"Opening the log folder failed: {error}");
        }
    }

    void OnCloseClick(object? sender, RoutedEventArgs e) => Close();
}
