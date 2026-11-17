#nullable enable
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Threading;
using System.Threading.Tasks;
using HLab.Mvvm.Annotations;

namespace LittleBigMouse.Plugins.Localization;

/// <summary>
/// Bridges <see cref="LbmLocalizer"/> into HLab's <see cref="ILocalizationService"/>.
/// </summary>
public sealed class LbmLocalizationProvider(LbmLocalizer localizer) : ILocalizationProvider
{
    public string Localize(string lang, string value) => localizer.GetForLanguage(lang, value);

    public Task<string> LocalizeAsync(string lang, string value, CancellationToken token = default)
        => Task.FromResult(localizer.GetForLanguage(lang, value));

    public ILocalizeEntry GetLocalizeEntry(string lang, string value)
        => new LocalizeEntryDesign(localizer.GetForLanguage(lang, value));

    public Task<ILocalizeEntry> GetLocalizeEntryAsync(string lang, string value)
        => Task.FromResult<ILocalizeEntry>(new LocalizeEntryDesign(localizer.GetForLanguage(lang, value)));

    public IEnumerable<ILocalizeEntry> GetLocalizeEntries(string value)
    {
        foreach (var lang in localizer.AvailableLanguages)
        {
            yield return new LocalizeEntryDesign(localizer.GetForLanguage(lang, value));
        }
    }

    public async IAsyncEnumerable<ILocalizeEntry> GetLocalizeEntriesAsync(
        string value,
        [EnumeratorCancellation] CancellationToken token = default)
    {
        foreach (var lang in localizer.AvailableLanguages)
        {
            if (token.IsCancellationRequested) yield break;
            yield return new LocalizeEntryDesign(localizer.GetForLanguage(lang, value));
        }
        await Task.CompletedTask;
    }

    public void Register(string tag, string code, string value, bool quality)
    {
    }
}
