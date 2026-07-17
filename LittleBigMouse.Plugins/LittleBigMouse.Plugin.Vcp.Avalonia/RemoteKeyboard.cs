#nullable enable
using System.Windows.Input;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Threading;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

/// <summary>
/// Describes the few remote-key names that differ between network remote protocols.
/// All desktop-key behavior stays shared.
/// </summary>
public sealed record RemoteKeyboardProfile(string ConfirmKey, string BackKey)
{
    public static RemoteKeyboardProfile Samsung { get; } = new("KEY_ENTER", "KEY_RETURN");
    public static RemoteKeyboardProfile Hisense { get; } = new("KEY_OK", "KEY_BACK");

    public string? RemoteKeyFor(Key key) => key switch
    {
        Key.Up => "KEY_UP",
        Key.Down => "KEY_DOWN",
        Key.Left => "KEY_LEFT",
        Key.Right => "KEY_RIGHT",
        Key.Enter => ConfirmKey,
        Key.Escape or Key.Back or Key.BrowserBack => BackKey,
        Key.Home => "KEY_HOME",
        Key.Apps => "KEY_MENU",
        Key.VolumeUp => "KEY_VOLUP",
        Key.VolumeDown => "KEY_VOLDOWN",
        Key.VolumeMute => "KEY_MUTE",
        Key.PageUp => "KEY_CHUP",
        Key.PageDown => "KEY_CHDOWN",
        Key.MediaPlayPause => "KEY_PLAYPAUSE",
        Key.MediaStop => "KEY_STOP",
        Key.MediaNextTrack => "KEY_FF",
        Key.MediaPreviousTrack => "KEY_REWIND",
        Key.MediaChannelRaise => "KEY_CHUP",
        Key.MediaChannelLower => "KEY_CHDOWN",
        Key.MediaHome => "KEY_HOME",
        Key.MediaMenu => "KEY_MENU",
        Key.MediaInfo => "KEY_INFO",
        Key.MediaRed => "KEY_RED",
        Key.MediaGreen => "KEY_GREEN",
        Key.MediaYellow => "KEY_YELLOW",
        Key.MediaBlue => "KEY_BLUE",
        Key.D0 or Key.NumPad0 => "KEY_0",
        Key.D1 or Key.NumPad1 => "KEY_1",
        Key.D2 or Key.NumPad2 => "KEY_2",
        Key.D3 or Key.NumPad3 => "KEY_3",
        Key.D4 or Key.NumPad4 => "KEY_4",
        Key.D5 or Key.NumPad5 => "KEY_5",
        Key.D6 or Key.NumPad6 => "KEY_6",
        Key.D7 or Key.NumPad7 => "KEY_7",
        Key.D8 or Key.NumPad8 => "KEY_8",
        Key.D9 or Key.NumPad9 => "KEY_9",
        _ => null,
    };
}

/// <summary>Gives any remote surface the same pointer focus and keyboard behavior.</summary>
public static class RemoteKeyboard
{
    public static void Attach(
        Control surface,
        RemoteKeyboardProfile profile,
        Func<ICommand?> getCommand)
    {
        // Wait until the complete click gesture has run. Moving focus during
        // PointerPressed cancels Avalonia buttons before they can raise Click.
        surface.AddHandler(
            InputElement.PointerReleasedEvent,
            (_, _) => Dispatcher.UIThread.Post(
                () => surface.Focus(),
                DispatcherPriority.Background),
            RoutingStrategies.Bubble,
            handledEventsToo: true);

        surface.AddHandler(
            InputElement.KeyDownEvent,
            (_, e) => SendKey(e, profile, getCommand),
            RoutingStrategies.Tunnel);
    }

    static void SendKey(KeyEventArgs e, RemoteKeyboardProfile profile, Func<ICommand?> getCommand)
    {
        if (e.KeyModifiers is not KeyModifiers.None) return;

        var remoteKey = profile.RemoteKeyFor(e.Key);
        var command = getCommand();
        if (remoteKey is null || command?.CanExecute(remoteKey) != true) return;

        command.Execute(remoteKey);
        e.Handled = true;
    }
}
