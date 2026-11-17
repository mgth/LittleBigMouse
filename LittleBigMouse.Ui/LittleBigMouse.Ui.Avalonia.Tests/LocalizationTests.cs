using System.Globalization;
using System.Reflection;
using System.Text.Json;
using Avalonia.Data;
using DynamicData.Binding;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia.Localization;
using LittleBigMouse.Plugins.Localization;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Avalonia.Options;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

public sealed class LocalizationTests : IDisposable
{
    readonly string _previousPreference = LbmLocalizer.Current.Preference;

    public void Dispose()
    {
        LbmLocalizer.Current.SetLanguage(_previousPreference, persist: false);
    }

    [Fact]
    public void EmbeddedCatalogsHaveIdenticalNonEmptyKeySets()
    {
        var assembly = typeof(LbmLocalizer).Assembly;
        var en = LoadCatalog(assembly, "en");
        var zh = LoadCatalog(assembly, "zh-CN");

        Assert.NotEmpty(en);
        Assert.Equal(en.Keys.OrderBy(k => k), zh.Keys.OrderBy(k => k));
        Assert.All(en, kv => Assert.False(string.IsNullOrWhiteSpace(kv.Value), $"Empty en value for '{kv.Key}'"));
        Assert.All(zh, kv => Assert.False(string.IsNullOrWhiteSpace(kv.Value), $"Empty zh-CN value for '{kv.Key}'"));
    }

    [Theory]
    [InlineData("zh-CN", "zh-CN")]
    [InlineData("zh-Hans", "zh-CN")]
    [InlineData("zh-SG", "zh-CN")]
    [InlineData("zh-TW", "zh-CN")]
    [InlineData("zh", "zh-CN")]
    [InlineData("en-US", "en")]
    [InlineData("fr-FR", "en")]
    public void ResolveLanguageMapsCulturesPredictably(string input, string expected)
    {
        Assert.Equal(expected, LbmLocalizer.ResolveLanguage(input, CultureInfo.InvariantCulture));
    }

    [Fact]
    public void AutoLanguageFollowsSystemUiCulture()
    {
        Assert.Equal("zh-CN", LbmLocalizer.ResolveLanguage("auto", new CultureInfo("zh-CN")));
        Assert.Equal("en", LbmLocalizer.ResolveLanguage("auto", new CultureInfo("en-US")));
    }

    [Fact]
    public void ObserveEmitsInitialAndUpdatedValuesUntilDisposed()
    {
        var localizer = new LbmLocalizer();
        var seen = new List<string>();
        var sub = localizer.Observe("Tray.Open").Subscribe(seen.Add);

        localizer.SetLanguage("zh-CN");
        sub.Dispose();
        localizer.SetLanguage("en");

        Assert.Equal(["Open", "打开"], seen);
    }

    [Fact]
    public void SwitchingLanguageUpdatesOptionsCaptionsWithoutChangingWireIds()
    {
        LbmLocalizer.Current.SetLanguage("en");
        using var vm = new LbmOptionsViewModel(new FakeProcessesCollector(), new FakeMainService(), new FakeDaemon());

        Assert.Equal("Corner crossing", vm.AlgorithmList.Single(i => i.Id == "Cross").Caption);

        LbmLocalizer.Current.SetLanguage("zh-CN");

        Assert.Equal(["Strait", "Cross"], vm.AlgorithmList.Select(i => i.Id).ToArray());
        Assert.NotEqual("Corner crossing", vm.AlgorithmList.Single(i => i.Id == "Cross").Caption);
    }

    [Fact]
    public void LanguagePreferenceRoundTripsInTempDirectory()
    {
        var tempDir = Path.Combine(Path.GetTempPath(), $"lbm-i18n-{Guid.NewGuid():N}");
        try
        {
            Assert.Equal(LbmLocalizer.AutoLanguage, LbmLocalizer.LoadLanguagePreference(tempDir));

            LbmLocalizer.SaveLanguagePreference("zh-CN", tempDir);
            Assert.Equal("zh-CN", LbmLocalizer.LoadLanguagePreference(tempDir));

            var localizer = new LbmLocalizer();
            localizer.Initialize(tempDir);
            Assert.Equal("zh-CN", localizer.EffectiveLanguage);
        }
        finally
        {
            if (Directory.Exists(tempDir)) Directory.Delete(tempDir, recursive: true);
        }
    }

    [Fact]
    public void TrExtensionProvidesAnAvaloniaBinding()
    {
        var ext = new TrExtension("Tray.Start");
        var provided = ext.ProvideValue(null!);
        Assert.IsAssignableFrom<BindingBase>(provided);
    }

    [Fact]
    public void GetOrFallbackReturnsLocalizedTextWhenPresentAndFallbackOtherwise()
    {
        var localizer = new LbmLocalizer();
        localizer.SetLanguage("zh-CN");

        Assert.Equal("显示方向", localizer.GetOrFallback("Debug.Field.DisplayOrientation", "DisplayOrientation"));
        Assert.Equal("CustomRawField", localizer.GetOrFallback("Debug.Field.CustomRawField", "CustomRawField"));
    }

    static Dictionary<string, string> LoadCatalog(Assembly assembly, string lang)
    {
        var suffix = $".Localization.Strings.{lang}.json";
        var name = assembly.GetManifestResourceNames()
            .Single(n => n.EndsWith(suffix, StringComparison.OrdinalIgnoreCase));
        using var stream = assembly.GetManifestResourceStream(name)!;
        return JsonSerializer.Deserialize<Dictionary<string, string>>(stream)!;
    }

    sealed class FakeProcessesCollector : IProcessesCollector
    {
        public ObservableCollectionExtended<string> SeenProcesses { get; } = [];
        public void AddProcess(string process) => SeenProcesses.Add(process);
    }

    sealed class FakeMainService : IMainService
    {
        public IMonitorsLayout MonitorsLayout { get; set; } =
            MainServiceFakes.NewLayout(new ILayoutOptions.Design());

        public bool LivePreview { get; set; }

        public void UpdateLayout() { }
        public void ReloadSystemLayout() { }
        public Task StartNotifierAsync() => Task.CompletedTask;
        public Task ShowControlAsync() => Task.CompletedTask;
        public void AddControlPlugin(Action<IMainPluginsViewModel>? action) { }
    }
}
