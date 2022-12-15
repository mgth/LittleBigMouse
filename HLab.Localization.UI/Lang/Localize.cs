using Avalonia;
using Avalonia.Controls;
using Avalonia.Metadata;
using Avalonia.Threading;
using HarfBuzzSharp;
using HLab.Base.Avalonia;
using HLab.Mvvm.Annotations;

namespace HLab.Localization.Avalonia.Lang;

using H = DependencyHelper<Localize>;

public class Localize : TextBlock
{
    public Localize()
    {
        if(Design.IsDesignMode)
            _updateAsync = UpdateDesignModeAsync;
        else
            _updateAsync = InitAsync;

    }

    [Content]
    public string Id
    {
        get => (string)GetValue(IdProperty);
        set => SetValue(IdProperty, value);
    }
    public static readonly StyledProperty<string> IdProperty =
        H.Property<string>()
            .OnChange(async (e, a) =>
            {

                await e._updateAsync();
            })
            .Register();

    public static readonly StyledProperty<string?> StringFormatProperty =
        H.Property<string?>()
//                .Default("{}{0}")
            .OnChangeBeforeNotification(async e =>
            {
                if (e.StringFormat == null)
                {
                    e._updateAsync = e.InitAsync;
                }
                await e._updateAsync();
            })
            .Register();

    public static readonly StyledProperty<ILocalizationService?> LocalizationServiceProperty =
        H.Property<ILocalizationService?>()
            .OnChange(async (e, a) =>
            {
                if (e.LocalizationService == null)
                {
                    e._updateAsync = e.InitAsync;
                }
                await e._updateAsync();
            })
            .Inherits
            .RegisterAttached();

    public static readonly StyledProperty<string?> LanguageProperty =
        H.Property<string?>()
            .OnChange(async (e, a) =>
            {
                if (e.Language == null)
                {
                    e._updateAsync = e.InitAsync;
                }
                await e._updateAsync();
            })
            .Inherits
            .RegisterAttached();


    public string? StringFormat
    {
        get => GetValue(StringFormatProperty);
        set => SetValue(StringFormatProperty, value);
    }

    public ILocalizationService? LocalizationService
    {
        get => GetValue(LocalizationServiceProperty);
        set => SetValue(LocalizationServiceProperty, value);
    } 
    
    public string? Language
    {
        get => GetValue(LanguageProperty);
        set => SetValue(LanguageProperty, value);
    }

    Func<Task> _updateAsync;

    public async Task InitAsync()
    {
        if (LocalizationService == null) return;
        if (Language == null) return;
        if (Id == null) return;
        _updateAsync = UpdateAsync;
        await UpdateAsync();
    }

    public async Task UpdateAsync()
    {
        var localized = Id;
        try
        {
            localized = await LocalizationService.LocalizeAsync(Language, localized).ConfigureAwait(false);
        }
        catch (Exception)
        {
        }

        await Dispatcher.UIThread.InvokeAsync(() => Text = localized);
    }
    public async Task UpdateDesignModeAsync()
    {
        await Dispatcher.UIThread.InvokeAsync(() => Text = Id);
    }


    public static ILocalizationService GetLocalizationService(StyledElement obj)
    {
        return (ILocalizationService)obj.GetValue(LocalizationServiceProperty);
    }
    public static void SetLocalizationService(StyledElement obj, ILocalizationService value)
    {
        obj.SetValue(LocalizationServiceProperty, value);
    }

    public static string GetLanguage(StyledElement obj)
    {
        return (string)obj.GetValue(LanguageProperty);
    }
    public static void SetLanguage(StyledElement obj, string value)
    {
        obj.SetValue(LanguageProperty, value);
    }
}