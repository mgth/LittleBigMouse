using Avalonia;
using Avalonia.Controls;
using Avalonia.Metadata;
using HLab.Base.Avalonia;

namespace HLab.Localization.Avalonia.Lang;

using H = DependencyHelper<LocalizedLabel>;
/// <summary>
/// Logique d'interaction pour LocalizedLabel.xaml
/// </summary>
public partial class LocalizedLabel : Label, INamed
{
    public LocalizedLabel()
    {
        InitializeComponent();
    }

    [Content]
    public string Text
    {
        get => (string)GetValue(TextProperty);
        set => SetValue(TextProperty, value);
    }

    public static readonly StyledProperty<string> TextProperty =
        H.Property<string>()
            .OnChangeBeforeNotification((e)=> e.Localize.Id = e.Text)
            .Register();
}