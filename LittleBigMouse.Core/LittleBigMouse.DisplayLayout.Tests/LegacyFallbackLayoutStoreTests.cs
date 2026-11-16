using LittleBigMouse.Plugins.Persistence;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// What the Windows window reads (v6, D2): the agent's JSON store, or the 5.x registry
/// while the agent has not imported it yet — the agent's marker being the absence of
/// options.json. The legacy store is a fake here: the registry one has its own tests.
/// </summary>
public class LegacyFallbackLayoutStoreTests : IDisposable
{
    readonly string _dir = Path.Combine(Path.GetTempPath(), "lbm-fallback-store-tests", Guid.NewGuid().ToString("N"));

    public void Dispose()
    {
        try { Directory.Delete(_dir, true); } catch { }
    }

    sealed class FakeLegacyStore : ILayoutStore
    {
        public int Reads { get; private set; }

        public LayoutStoreData Read(string layoutId, IReadOnlyCollection<string> pnpCodes)
        {
            Reads++;
            return new LayoutStoreData { GlobalOptions = new GlobalOptionsDto { Priority = "FromRegistry" } };
        }

        public void WriteGlobalOptions(GlobalOptionsDto options) => throw new InvalidOperationException("the registry is never written");
        public void WriteLayout(string layoutId, LayoutDto layout) => throw new InvalidOperationException("the registry is never written");
        public void WriteModels(IReadOnlyDictionary<string, ModelDto> models) => throw new InvalidOperationException("the registry is never written");
    }

    [Fact]
    public void Before_the_import_the_registry_is_read()
    {
        var legacy = new FakeLegacyStore();
        var store = new LegacyFallbackLayoutStore(new JsonLayoutStore(_dir), () => legacy);

        var data = store.Read("LAYOUT1", []);

        Assert.Equal("FromRegistry", data.GlobalOptions!.Priority);
        Assert.Equal(1, legacy.Reads);
    }

    [Fact]
    public void Once_the_agent_holds_options_its_store_is_read_and_the_registry_is_not_opened()
    {
        var json = new JsonLayoutStore(_dir);
        json.WriteGlobalOptions(new GlobalOptionsDto { Priority = "FromAgent" });
        json.WriteLayout("LAYOUT1", new LayoutDto { Options = new LayoutOptionsDto { Enabled = true } });
        var opened = 0;
        var store = new LegacyFallbackLayoutStore(json, () => { opened++; return new FakeLegacyStore(); });

        var data = store.Read("LAYOUT1", []);

        Assert.Equal("FromAgent", data.GlobalOptions!.Priority);
        Assert.True(data.Layout!.Options!.Enabled);
        Assert.Equal(0, opened);
    }

    [Fact]
    public void Writes_go_to_the_agent_store_only()
    {
        var json = new JsonLayoutStore(_dir);
        var store = new LegacyFallbackLayoutStore(json, () => new FakeLegacyStore());

        store.WriteGlobalOptions(new GlobalOptionsDto { Priority = "Written" });
        store.WriteLayout("LAYOUT1", new LayoutDto());
        store.WriteModels(new Dictionary<string, ModelDto> { ["TST1234"] = new() });

        Assert.True(json.HoldsOptions);
        Assert.Equal("Written", store.Read("LAYOUT1", []).GlobalOptions!.Priority);
    }
}
