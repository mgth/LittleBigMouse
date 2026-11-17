#nullable enable
using System;
using Avalonia;
using Avalonia.Data;
using Avalonia.Markup.Xaml;
using LittleBigMouse.Plugins.Localization;

namespace LittleBigMouse.Plugins.Avalonia.Localization;

/// <summary>
/// Avalonia XAML markup extension that binds any string/object property to
/// <see cref="LbmLocalizer.Current"/> and updates live when the language changes.
/// Usage: <c>Text="{loc:Tr KeyName}"</c>
/// </summary>
public sealed class TrExtension : MarkupExtension
{
    public TrExtension()
    {
    }

    public TrExtension(string key)
    {
        Key = key;
    }

    public string Key { get; set; } = "";

    public override object ProvideValue(IServiceProvider serviceProvider)
        => LbmLocalizer.Current.Observe(Key).ToBinding();
}
