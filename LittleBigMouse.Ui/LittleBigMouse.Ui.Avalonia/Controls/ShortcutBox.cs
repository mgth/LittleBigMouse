using System;
using System.Collections.Generic;
using System.Linq;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Data;
using Avalonia.Input;

namespace LittleBigMouse.Ui.Avalonia.Controls;

/// <summary>
/// Records a keyboard shortcut by having the user press it.
/// <para>
/// Typing one out is a trap: the daemon can only register what Windows understands, so
/// a name it cannot map would be accepted here and fail silently over there, far from
/// its cause. Pressing the combination cannot produce anything the keyboard cannot
/// produce — and the writing-out is done here, once, in the grammar both sides obey
/// (LittleBigMouse-Hook-Rust/src/shortcut.rs).
/// </para>
/// </summary>
public class ShortcutBox : Button
{
    public static readonly StyledProperty<string> ShortcutProperty =
        AvaloniaProperty.Register<ShortcutBox, string>(
            nameof(Shortcut), defaultBindingMode: BindingMode.TwoWay);

    public static readonly StyledProperty<bool> IsRecordingProperty =
        AvaloniaProperty.Register<ShortcutBox, bool>(nameof(IsRecording));

    /// <summary>The combination, as it reads and as it travels: "Ctrl+Alt+Shift+M".</summary>
    public string Shortcut
    {
        get => GetValue(ShortcutProperty);
        set => SetValue(ShortcutProperty, value);
    }

    /// <summary>Waiting for a combination. Styling hangs off this.</summary>
    public bool IsRecording
    {
        get => GetValue(IsRecordingProperty);
        private set => SetValue(IsRecordingProperty, value);
    }

    // No StyleKeyOverride: the caption and the waiting state come from this control's
    // own ControlTheme (App.axaml), which is looked up by type. Pointing the style key
    // at Button would borrow Button's look and, with it, make every selector written
    // against ShortcutBox miss — a blank button with no styling and no explanation.

    public ShortcutBox()
    {
        Click += (_, _) => IsRecording = !IsRecording;
        LostFocus += (_, _) => IsRecording = false;
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        if (!IsRecording)
        {
            base.OnKeyDown(e);
            return;
        }

        // Nothing reaches the button while recording — not Space or Enter, which would
        // otherwise re-trigger Click and cancel the recording the user just started.
        e.Handled = true;

        // A modifier on its own is the user still reaching for the key.
        if (IsModifier(e.Key)) return;

        if (e.Key == Key.Escape)
        {
            IsRecording = false;
            return;
        }

        var name = KeyName(e.Key);
        if (name is null) return;

        // At least one modifier, always. Registered globally, a bare key would be taken
        // away from every application on the desktop — the daemon refuses one too, and
        // refusing it here is what makes that refusal visible instead of silent.
        var modifiers = ModifierNames(e.KeyModifiers).ToList();
        if (modifiers.Count == 0) return;

        Shortcut = string.Join("+", modifiers.Append(name));
        IsRecording = false;
    }

    internal static bool IsModifier(Key key) => key
        is Key.LeftCtrl or Key.RightCtrl
        or Key.LeftAlt or Key.RightAlt
        or Key.LeftShift or Key.RightShift
        or Key.LWin or Key.RWin;

    /// <summary>Modifiers in a fixed order, so the same combination always reads the same.</summary>
    internal static IEnumerable<string> ModifierNames(KeyModifiers modifiers)
    {
        if (modifiers.HasFlag(KeyModifiers.Control)) yield return "Ctrl";
        if (modifiers.HasFlag(KeyModifiers.Alt)) yield return "Alt";
        if (modifiers.HasFlag(KeyModifiers.Shift)) yield return "Shift";
        if (modifiers.HasFlag(KeyModifiers.Meta)) yield return "Win";
    }

    /// <summary>
    /// The keys a shortcut may use, named as the daemon's parser reads them. Closed on
    /// purpose, and it is the same closed set on both sides: a key recorded here that
    /// the daemon cannot register would be a shortcut that quietly does not exist.
    /// </summary>
    internal static string? KeyName(Key key) => key switch
    {
        >= Key.A and <= Key.Z => key.ToString(),
        >= Key.D0 and <= Key.D9 => key.ToString()[1..],
        >= Key.F1 and <= Key.F24 => key.ToString(),
        Key.Space => "Space",
        Key.Enter => "Enter",
        Key.Tab => "Tab",
        Key.Back => "Backspace",
        Key.Insert => "Insert",
        Key.Delete => "Delete",
        Key.Home => "Home",
        Key.End => "End",
        Key.PageUp => "PageUp",
        Key.PageDown => "PageDown",
        Key.Left => "Left",
        Key.Up => "Up",
        Key.Right => "Right",
        Key.Down => "Down",
        Key.Pause => "Pause",
        Key.PrintScreen => "PrintScreen",

        // Punctuation, by its position rather than by what it prints: the same physical
        // key is ² on an AZERTY board and ` on a QWERTY one, and the daemon registers a
        // position. Leaving these out amputated every non-alphanumeric key, which on a
        // French keyboard is most of the interesting ones.
        Key.OemSemicolon => "Oem1",
        Key.OemQuestion => "Oem2",
        Key.OemTilde => "Oem3",
        Key.OemOpenBrackets => "Oem4",
        Key.OemPipe => "Oem5",
        Key.OemCloseBrackets => "Oem6",
        Key.OemQuotes => "Oem7",
        Key.Oem8 => "Oem8",
        Key.OemBackslash => "Oem102",
        Key.OemPlus => "OemPlus",
        Key.OemComma => "OemComma",
        Key.OemMinus => "OemMinus",
        Key.OemPeriod => "OemPeriod",

        >= Key.NumPad0 and <= Key.NumPad9 => key.ToString(),
        Key.Multiply => "NumPadMultiply",
        Key.Add => "NumPadAdd",
        Key.Subtract => "NumPadSubtract",
        Key.Decimal => "NumPadDecimal",
        Key.Divide => "NumPadDivide",

        _ => null,
    };

    /// <summary>
    /// The Win32 virtual-key code behind a position name, for the position-named keys
    /// only — the ones <see cref="ShortcutDisplay"/> has to turn back into whatever
    /// character the current layout prints on them.
    /// <para>
    /// These numbers are the daemon's too (shortcut.rs::virtual_key). They are asserted
    /// on both sides for the same reason the names are: nothing else would notice them
    /// drifting apart.
    /// </para>
    /// </summary>
    internal static uint? VirtualKey(string name) => name switch
    {
        "Oem1" => 0xBA,
        "Oem2" => 0xBF,
        "Oem3" => 0xC0,
        "Oem4" => 0xDB,
        "Oem5" => 0xDC,
        "Oem6" => 0xDD,
        "Oem7" => 0xDE,
        "Oem8" => 0xDF,
        "Oem102" => 0xE2,
        "OemPlus" => 0xBB,
        "OemComma" => 0xBC,
        "OemMinus" => 0xBD,
        "OemPeriod" => 0xBE,
        _ => null,
    };
}
