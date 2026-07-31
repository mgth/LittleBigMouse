using Avalonia.Input;
using LittleBigMouse.Ui.Avalonia.Controls;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The recorder writes a shortcut; the daemon reads it. They are in different
/// languages and different processes, so the grammar between them is a contract with
/// no compiler behind it — these tests are that compiler.
/// <para>
/// The other half lives in <c>LittleBigMouse-Hook-Rust/src/shortcut.rs</c>. A key
/// added on one side and not the other is a shortcut the user can record and that
/// quietly never fires, which they would discover at the worst possible moment.
/// </para>
/// </summary>
public sealed class ShortcutBoxTests
{
    [Theory]
    [InlineData(Key.M, "M")]
    [InlineData(Key.A, "A")]
    [InlineData(Key.Z, "Z")]
    [InlineData(Key.D0, "0")]
    [InlineData(Key.D7, "7")]
    [InlineData(Key.F1, "F1")]
    [InlineData(Key.F12, "F12")]
    [InlineData(Key.F24, "F24")]
    [InlineData(Key.Space, "Space")]
    [InlineData(Key.Escape, null)]      // reserved: it cancels the recording
    [InlineData(Key.Enter, "Enter")]
    [InlineData(Key.Tab, "Tab")]
    [InlineData(Key.Back, "Backspace")]
    [InlineData(Key.Insert, "Insert")]
    [InlineData(Key.Delete, "Delete")]
    [InlineData(Key.Home, "Home")]
    [InlineData(Key.End, "End")]
    [InlineData(Key.PageUp, "PageUp")]
    [InlineData(Key.PageDown, "PageDown")]
    [InlineData(Key.Left, "Left")]
    [InlineData(Key.Up, "Up")]
    [InlineData(Key.Right, "Right")]
    [InlineData(Key.Down, "Down")]
    [InlineData(Key.Pause, "Pause")]
    [InlineData(Key.PrintScreen, "PrintScreen")]
    public void KeysAreNamedTheWayTheDaemonReadsThem(Key key, string? expected)
    {
        Assert.Equal(expected, ShortcutBox.KeyName(key));
    }

    [Theory]
    // Punctuation and the keypad — half of what a French keyboard offers. Every string
    // here must be one the daemon's parser reads (shortcut.rs::virtual_key); that
    // agreement has no compiler behind it, only these two test files.
    [InlineData(Key.OemSemicolon, "Oem1")]
    [InlineData(Key.OemQuestion, "Oem2")]
    [InlineData(Key.OemTilde, "Oem3")]
    [InlineData(Key.OemOpenBrackets, "Oem4")]
    [InlineData(Key.OemPipe, "Oem5")]
    [InlineData(Key.OemCloseBrackets, "Oem6")]
    [InlineData(Key.OemQuotes, "Oem7")]
    [InlineData(Key.Oem8, "Oem8")]
    [InlineData(Key.OemBackslash, "Oem102")]
    [InlineData(Key.OemPlus, "OemPlus")]
    [InlineData(Key.OemComma, "OemComma")]
    [InlineData(Key.OemMinus, "OemMinus")]
    [InlineData(Key.OemPeriod, "OemPeriod")]
    [InlineData(Key.NumPad0, "NumPad0")]
    [InlineData(Key.NumPad9, "NumPad9")]
    [InlineData(Key.Multiply, "NumPadMultiply")]
    [InlineData(Key.Add, "NumPadAdd")]
    [InlineData(Key.Subtract, "NumPadSubtract")]
    [InlineData(Key.Decimal, "NumPadDecimal")]
    [InlineData(Key.Divide, "NumPadDivide")]
    public void PunctuationAndKeypadAreRecordable(Key key, string expected)
    {
        Assert.Equal(expected, ShortcutBox.KeyName(key));
    }

    [Theory]
    // Keys with no virtual-key mapping on the daemon's side. Recording one would
    // produce a shortcut that cannot be registered, so it must not be recordable.
    [InlineData(Key.LeftCtrl)]
    [InlineData(Key.CapsLock)]
    [InlineData(Key.NumLock)]
    [InlineData(Key.VolumeUp)]
    [InlineData(Key.MediaPlayPause)]
    [InlineData(Key.ImeConvert)]
    public void KeysTheDaemonCannotRegisterAreNotRecordable(Key key)
    {
        Assert.Null(ShortcutBox.KeyName(key));
    }

    [Fact]
    public void ModifiersAlwaysReadInTheSameOrder()
    {
        // So the same combination produces the same string every time — otherwise a
        // re-record would look like a change, and the daemon would re-register for
        // nothing.
        Assert.Equal(
            new[] { "Ctrl", "Alt", "Shift", "Win" },
            ShortcutBox.ModifierNames(
                KeyModifiers.Shift | KeyModifiers.Meta | KeyModifiers.Control | KeyModifiers.Alt));

        Assert.Equal(
            new[] { "Ctrl", "Shift" },
            ShortcutBox.ModifierNames(KeyModifiers.Shift | KeyModifiers.Control));
    }

    [Theory]
    // The daemon's numbers (shortcut.rs::virtual_key), asserted here too. Only the
    // position-named keys need one on this side, to turn back into a character for
    // display — but a number that drifted would send the display and the registration
    // to two different keys, and nothing else would notice.
    [InlineData("Oem1", 0xBA)]
    [InlineData("Oem2", 0xBF)]
    [InlineData("Oem3", 0xC0)]
    [InlineData("Oem4", 0xDB)]
    [InlineData("Oem5", 0xDC)]
    [InlineData("Oem6", 0xDD)]
    [InlineData("Oem7", 0xDE)]
    [InlineData("Oem8", 0xDF)]
    [InlineData("Oem102", 0xE2)]
    [InlineData("OemPlus", 0xBB)]
    [InlineData("OemComma", 0xBC)]
    [InlineData("OemMinus", 0xBD)]
    [InlineData("OemPeriod", 0xBE)]
    public void PositionNamesCarryTheDaemonsVirtualKeyCodes(string name, uint vk)
    {
        Assert.Equal(vk, ShortcutBox.VirtualKey(name));
    }

    [Fact]
    public void OnlyPositionNamesHaveAVirtualKeyOnThisSide()
    {
        // The rest already read as themselves and are never translated for display.
        Assert.Null(ShortcutBox.VirtualKey("M"));
        Assert.Null(ShortcutBox.VirtualKey("F12"));
        Assert.Null(ShortcutBox.VirtualKey("NumPad0"));
        Assert.Null(ShortcutBox.VirtualKey("Oem9"));
    }

    [Fact]
    public void ReadableLeavesEverythingItCannotImproveAlone()
    {
        // Layout-independent assertions only: what Oem7 prints is ² here and ' on the
        // CI runner, so the test pins the shape, not the character.
        Assert.Equal("Ctrl+Alt+Shift+M", ShortcutDisplay.Readably("Ctrl+Alt+Shift+M"));
        Assert.Equal("Ctrl+F12", ShortcutDisplay.Readably("Ctrl+F12"));
        Assert.Equal("Ctrl+NumPad0", ShortcutDisplay.Readably("Ctrl+NumPad0"));
        Assert.Equal("", ShortcutDisplay.Readably(""));
        Assert.Equal("", ShortcutDisplay.Readably(null));
    }

    [Fact]
    public void ReadableTurnsAPositionIntoWhatTheKeyboardPrints()
    {
        var readable = ShortcutDisplay.Readably("Ctrl+Oem7");

        Assert.StartsWith("Ctrl+", readable);
        var key = readable["Ctrl+".Length..];

        // One character on a layout that prints something there, the position name on
        // one that does not — never blank, and never half-translated.
        Assert.True(key.Length == 1 || key == "Oem7", $"unexpected rendering: {key}");
    }

    [Fact]
    public void EveryModifierKeyIsRecognizedAsOne()
    {
        // Left and right alike: pressing one is the user still reaching for the key,
        // not a combination to record.
        foreach (var key in new[]
                 {
                     Key.LeftCtrl, Key.RightCtrl, Key.LeftAlt, Key.RightAlt,
                     Key.LeftShift, Key.RightShift, Key.LWin, Key.RWin,
                 })
        {
            Assert.True(ShortcutBox.IsModifier(key), $"{key} must not end a recording");
        }

        Assert.False(ShortcutBox.IsModifier(Key.M));
    }
}
