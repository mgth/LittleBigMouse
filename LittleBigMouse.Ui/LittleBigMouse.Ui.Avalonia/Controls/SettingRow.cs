using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;

namespace LittleBigMouse.Ui.Avalonia.Controls;

/// <summary>
/// A settings-panel row: optional icon, header with optional description,
/// and a right-aligned control provided as Content.
/// </summary>
public class SettingRow : ContentControl
{
    public static readonly StyledProperty<Geometry?> IconProperty =
        AvaloniaProperty.Register<SettingRow, Geometry?>(nameof(Icon));

    public static readonly StyledProperty<string?> HeaderProperty =
        AvaloniaProperty.Register<SettingRow, string?>(nameof(Header));

    public static readonly StyledProperty<string?> DescriptionProperty =
        AvaloniaProperty.Register<SettingRow, string?>(nameof(Description));

    public Geometry? Icon
    {
        get => GetValue(IconProperty);
        set => SetValue(IconProperty, value);
    }

    public string? Header
    {
        get => GetValue(HeaderProperty);
        set => SetValue(HeaderProperty, value);
    }

    public string? Description
    {
        get => GetValue(DescriptionProperty);
        set => SetValue(DescriptionProperty, value);
    }
}
