#nullable enable
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.ComponentModel;
using System.Globalization;
using System.IO;
using System.Reflection;
using System.Text.Json;

namespace LittleBigMouse.Plugins.Localization;

/// <summary>
/// Central localization store for LittleBigMouse UI and plugins.
/// Defaults to English until <see cref="Initialize"/> or <see cref="SetLanguage"/> is called,
/// keeping unit tests deterministic regardless of the host OS locale.
/// </summary>
public sealed class LbmLocalizer : INotifyPropertyChanged
{
    public const string AutoLanguage = "auto";
    public const string DefaultLanguage = "en";
    public const string SimplifiedChinese = "zh-CN";

    static readonly string[] SupportedLanguages = [DefaultLanguage, SimplifiedChinese];

    readonly Assembly _resourceAssembly;
    readonly ConcurrentDictionary<string, IReadOnlyDictionary<string, string>> _catalogs = new(StringComparer.OrdinalIgnoreCase);
    readonly object _gate = new();

    string _preference = DefaultLanguage;
    string _effectiveLanguage = DefaultLanguage;

    public static LbmLocalizer Current { get; } = new();

    public LbmLocalizer() : this(typeof(LbmLocalizer).Assembly)
    {
    }

    internal LbmLocalizer(Assembly resourceAssembly)
    {
        _resourceAssembly = resourceAssembly;
    }

    public event PropertyChangedEventHandler? PropertyChanged;
    public event EventHandler? LanguageChanged;

    public string Preference
    {
        get
        {
            lock (_gate) return _preference;
        }
    }

    public string EffectiveLanguage
    {
        get
        {
            lock (_gate) return _effectiveLanguage;
        }
    }

    public string this[string key] => Get(key);

    public IReadOnlyList<string> AvailableLanguages => SupportedLanguages;

    /// <summary>
    /// Initializes the localizer from the persisted user preference (falling back to
    /// <see cref="AutoLanguage"/> when no preference file exists).
    /// </summary>
    public void Initialize(string? configDir = null)
    {
        var preference = LoadLanguagePreference(configDir);
        SetLanguage(preference, persist: false, configDir: configDir);
    }

    public void SetLanguage(string? preference, bool persist = false, string? configDir = null)
    {
        var normalizedPref = NormalizePreference(preference);
        var resolved = ResolveLanguage(normalizedPref, CultureInfo.CurrentUICulture);

        bool changed;
        lock (_gate)
        {
            changed = !string.Equals(_preference, normalizedPref, StringComparison.OrdinalIgnoreCase)
                || !string.Equals(_effectiveLanguage, resolved, StringComparison.OrdinalIgnoreCase);
            _preference = normalizedPref;
            _effectiveLanguage = resolved;
        }

        if (persist)
        {
            SaveLanguagePreference(normalizedPref, configDir);
        }

        if (changed)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(Preference)));
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(EffectiveLanguage)));
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs("Item"));
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs("Item[]"));
            LanguageChanged?.Invoke(this, EventArgs.Empty);
        }
    }

    public string Get(string key) => GetForLanguage(EffectiveLanguage, key);

    public bool TryGet(string key, out string value) => TryGetForLanguage(EffectiveLanguage, key, out value);

    public string GetOrFallback(string key, string fallback)
        => TryGet(key, out var value) ? value : fallback;

    public string GetForLanguage(string? languageTag, string key)
        => TryGetForLanguage(languageTag, key, out var value) ? value : key;

    public bool TryGetForLanguage(string? languageTag, string key, out string value)
    {
        if (string.IsNullOrEmpty(key))
        {
            value = "";
            return false;
        }

        var lang = ResolveLanguage(languageTag, CultureInfo.CurrentUICulture);
        var catalog = LoadCatalog(lang);
        if (catalog.TryGetValue(key, out var localized) && !string.IsNullOrEmpty(localized))
        {
            value = localized;
            return true;
        }

        if (!string.Equals(lang, DefaultLanguage, StringComparison.OrdinalIgnoreCase))
        {
            var fallback = LoadCatalog(DefaultLanguage);
            if (fallback.TryGetValue(key, out var english) && !string.IsNullOrEmpty(english))
            {
                value = english;
                return true;
            }
        }

        value = key;
        return false;
    }

    public string Format(string key, params object?[] args)
    {
        var template = Get(key);
        if (args.Length == 0) return template;

        try
        {
            return string.Format(CultureInfo.CurrentCulture, template, args);
        }
        catch (FormatException)
        {
            return template;
        }
    }

    public IObservable<string> Observe(string key) => new LocalizedObservable(this, () => Get(key));

    public IObservable<string> ObserveOrFallback(string key, string fallback)
        => new LocalizedObservable(this, () => GetOrFallback(key, fallback));

    public IObservable<string> Observe(Func<string> selector) => new LocalizedObservable(this, selector);

    public static string NormalizePreference(string? preference)
    {
        if (string.IsNullOrWhiteSpace(preference)) return AutoLanguage;
        var trimmed = preference.Trim();
        if (string.Equals(trimmed, AutoLanguage, StringComparison.OrdinalIgnoreCase)) return AutoLanguage;
        return ResolveLanguage(trimmed, CultureInfo.CurrentUICulture);
    }

    public static string ResolveLanguage(string? tag, CultureInfo systemUiCulture)
    {
        var candidate = tag?.Trim();
        if (string.IsNullOrEmpty(candidate) || string.Equals(candidate, AutoLanguage, StringComparison.OrdinalIgnoreCase))
        {
            candidate = systemUiCulture.Name;
        }

        var lower = candidate.ToLowerInvariant();
        if (lower == "zh" || lower == "zh-cn" || lower == "zh-sg" || lower.StartsWith("zh-hans", StringComparison.Ordinal))
        {
            return SimplifiedChinese;
        }

        if (lower == "zh-tw" || lower == "zh-hk" || lower == "zh-mo" || lower.StartsWith("zh-hant", StringComparison.Ordinal))
        {
            return SimplifiedChinese;
        }

        if (lower == "en" || lower.StartsWith("en-", StringComparison.Ordinal))
        {
            return DefaultLanguage;
        }

        return DefaultLanguage;
    }

    static string PreferenceFilePath(string? configDir)
        => Path.Combine(configDir ?? LbmPaths.ConfigDir, "language.txt");

    public static string LoadLanguagePreference(string? configDir = null)
    {
        try
        {
            var path = PreferenceFilePath(configDir);
            if (!File.Exists(path)) return AutoLanguage;
            var content = File.ReadAllText(path).Trim();
            return string.IsNullOrEmpty(content) ? AutoLanguage : NormalizePreference(content);
        }
        catch
        {
            return AutoLanguage;
        }
    }

    public static void SaveLanguagePreference(string preference, string? configDir = null)
    {
        try
        {
            var path = PreferenceFilePath(configDir);
            var dir = Path.GetDirectoryName(path);
            if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);
            File.WriteAllText(path, NormalizePreference(preference));
        }
        catch
        {
            // Non-fatal: UI language preference should never crash startup or settings.
        }
    }

    IReadOnlyDictionary<string, string> LoadCatalog(string language)
    {
        return _catalogs.GetOrAdd(language, lang =>
        {
            var suffix = $".Localization.Strings.{lang}.json";
            var resourceName = Array.Find(
                _resourceAssembly.GetManifestResourceNames(),
                n => n.EndsWith(suffix, StringComparison.OrdinalIgnoreCase));

            if (resourceName is null) return new Dictionary<string, string>(StringComparer.Ordinal);

            using var stream = _resourceAssembly.GetManifestResourceStream(resourceName);
            if (stream is null) return new Dictionary<string, string>(StringComparer.Ordinal);

            var dict = JsonSerializer.Deserialize<Dictionary<string, string>>(stream);
            return dict is null
                ? new Dictionary<string, string>(StringComparer.Ordinal)
                : new Dictionary<string, string>(dict, StringComparer.Ordinal);
        });
    }

    sealed class LocalizedObservable(LbmLocalizer owner, Func<string> selector) : IObservable<string>
    {
        public IDisposable Subscribe(IObserver<string> observer)
        {
            observer.OnNext(selector());
            return new Subscription(owner, observer, selector);
        }

        sealed class Subscription : IDisposable
        {
            readonly LbmLocalizer _owner;
            readonly IObserver<string> _observer;
            readonly Func<string> _selector;
            int _disposed;

            public Subscription(LbmLocalizer owner, IObserver<string> observer, Func<string> selector)
            {
                _owner = owner;
                _observer = observer;
                _selector = selector;
                _owner.LanguageChanged += OnLanguageChanged;
            }

            void OnLanguageChanged(object? sender, EventArgs e)
            {
                if (System.Threading.Volatile.Read(ref _disposed) != 0) return;
                _observer.OnNext(_selector());
            }

            public void Dispose()
            {
                if (System.Threading.Interlocked.Exchange(ref _disposed, 1) != 0) return;
                _owner.LanguageChanged -= OnLanguageChanged;
            }
        }
    }
}
