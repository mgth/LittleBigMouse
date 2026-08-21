using System.Globalization;
using LittleBigMouse.Platform.Windows;
using LittleBigMouse.Plugins.Persistence;
using Microsoft.Win32;

// Every test here is a [WindowsFact]; the analyzer cannot see that.
#pragma warning disable CA1416

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The Windows backend against a real registry tree, under a throwaway root key.
/// <para>
/// This is the twin of <see cref="JsonLayoutStoreTests"/>, and it was the blind spot: the
/// key names, the "1"/"0" bools, the invariant numbers and the legacy locations ARE the
/// stored format, and nothing exercised them. <c>RescueShortcut</c> reached the mapping
/// and never the registry for several releases without a single test noticing.
/// </para>
/// <para>
/// The root key is injected (<c>SOFTWARE\Mgth\LittleBigMouse-Tests\{guid}</c>) so a run
/// can never touch the settings of whoever runs the suite, and the tree is deleted after
/// each test.
/// </para>
/// </summary>
public class RegistryLayoutStoreTests : IDisposable
{
    const string LayoutId = "L1";
    const string MonitorId = "MON1";
    const string SourceId = "DISPLAY1";
    const string Pnp = "TST1234";

    // Next to the real key rather than somewhere else in HKCU: a leaked test tree is then
    // obvious, and deleting this path can never reach the user's own settings.
    readonly string _root = $@"SOFTWARE\Mgth\LittleBigMouse-Tests\{Guid.NewGuid():N}";

    RegistryLayoutStore Store => new(_root);

    public void Dispose()
    {
        if (!OperatingSystem.IsWindows()) return;
        try { Registry.CurrentUser.DeleteSubKeyTree(_root, throwOnMissingSubKey: false); } catch { }
    }

    //==================//
    // Completeness     //
    //==================//

    [WindowsFact]
    public void GlobalOptions_EveryPropertyRoundTrips()
    {
        // Filled by reflection ON PURPOSE: a property added to the DTO is covered here the
        // day it is added, with nobody having to remember this file. That is exactly what
        // was missing while RescueShortcut sat in the mapping and never in the registry.
        var written = Filled<GlobalOptionsDto>();

        Store.WriteGlobalOptions(written);

        AssertSameProperties(written, Store.Read("ANY", []).GlobalOptions!);
    }

    [WindowsFact]
    public void LayoutOptions_EveryPropertyRoundTrips_ExceptTheLegacyPriorities()
    {
        var written = Filled<LayoutOptionsDto>();

        Store.WriteLayout(LayoutId, new LayoutDto { Options = written });
        var read = Store.Read(LayoutId, []).Layout!.Options!;

        AssertSameProperties(written, read,
            nameof(LayoutOptionsDto.Priority), nameof(LayoutOptionsDto.PriorityUnhooked));

        // Those two are app-level options a layout may still carry: read as a fallback,
        // never written back. See WriteLayout_DeletesTheLegacyPriorityFromTheLayoutKey.
        Assert.Null(read.Priority);
        Assert.Null(read.PriorityUnhooked);
    }

    [WindowsFact]
    public void Monitor_RoundTripsGeometryBordersAndSources()
    {
        var borders = Filled<BordersDto>();
        var source = Filled<SourceDto>();
        var section = Filled<BorderSectionDto>();

        var monitor = new MonitorDto
        {
            XLocationInMm = 320.5,
            YLocationInMm = -12,
            PhysicalRatioX = 1,
            PhysicalRatioY = 1.25,
            BorderResistance = new BorderResistanceDto { Left = new BorderSideDto { Sections = [section] } },
            Borders = borders,
            ActiveSource = SourceId,
            SerialNumber = "SN-0001",
            ExcludedFromLayout = true,
            Sources = new Dictionary<string, SourceDto> { [SourceId] = source }
        };

        WriteMonitor(monitor);
        var read = ReadMonitor();

        AssertSameProperties(monitor, read,
            nameof(MonitorDto.BorderResistance), nameof(MonitorDto.Borders), nameof(MonitorDto.Sources));
        AssertSameProperties(borders, read.Borders!);
        AssertSameProperties(source, read.Sources![SourceId]);
        AssertSameProperties(section, Assert.Single(read.BorderResistance!.Left!.Sections!));
    }

    [WindowsFact]
    public void Model_RoundTripsSizeBordersAndName()
    {
        var borders = Filled<BordersDto>();
        var model = new ModelDto { Width = 598, Height = 338, Borders = borders, PnpName = "Test Monitor 24" };

        Store.WriteModels(new Dictionary<string, ModelDto> { [Pnp] = model });
        var read = Store.Read("ANY", [Pnp]).Models[Pnp];

        AssertSameProperties(model, read, nameof(ModelDto.Borders));
        AssertSameProperties(borders, read.Borders!);
    }

    [WindowsFact]
    public void BorderSide_RoundTripsItsOwnFieldsAndItsSections()
    {
        // Move/Drag are no longer written by the mapper, but the store still has to carry
        // them: it is the layer that must not lose what it is handed.
        var side = new BorderSideDto
        {
            Move = 1.5,
            MoveBlock = true,
            Drag = 20,
            DragBlock = false,
            Sections =
            [
                new BorderSectionDto { From = 0, To = 100, Move = 2, MoveBlock = false, Drag = 3, DragBlock = true },
                new BorderSectionDto { From = 100, To = 250, Move = 4, MoveBlock = true, Drag = 5, DragBlock = false }
            ]
        };

        WriteMonitor(new MonitorDto { BorderResistance = new BorderResistanceDto { Right = side } });
        var read = ReadMonitor().BorderResistance!.Right!;

        Assert.Equal(1.5, read.Move);
        Assert.True(read.MoveBlock);
        Assert.Equal(20, read.Drag);
        Assert.False(read.DragBlock);

        Assert.Equal(2, read.Sections!.Count);
        AssertSameProperties(side.Sections[0], read.Sections[0]);
        AssertSameProperties(side.Sections[1], read.Sections[1]);
    }

    [WindowsFact]
    public void Sections_ComeBackInOrder_NotAlphabetically()
    {
        // Sections are subkeys named by index and GetSubKeyNames sorts as text, where
        // "10" comes before "2". Ten is the smallest list that shows it.
        List<BorderSectionDto> sections =
            [.. Enumerable.Range(0, 12).Select(i => new BorderSectionDto { From = i, To = i + 1 })];

        WriteMonitor(new MonitorDto
        {
            BorderResistance = new BorderResistanceDto { Top = new BorderSideDto { Sections = sections } }
        });

        var read = ReadMonitor().BorderResistance!.Top!.Sections!;
        Assert.Equal(sections.Select(s => s.From), read.Select(s => s.From));
    }

    //==================//
    // Legacy locations //
    //==================//

    [WindowsFact]
    public void ReadSide_PrefersThePreSplitValueOverTheSubkey()
    {
        // Before the move/drag split, an edge was a single VALUE holding its resistance.
        // Reading it is what migrates an installation: one number meant "resist any
        // crossing", so it maps to both modes. Twin of the JSON converter's legacy shape.
        using (var key = CreateKey($@"Layouts\{LayoutId}\PhysicalMonitors\{MonitorId}\BorderResistance"))
            key.SetValue("Left", "20", RegistryValueKind.String);

        var side = ReadMonitor().BorderResistance!.Left!;

        Assert.Equal(20, side.Move);
        Assert.Equal(20, side.Drag);
        Assert.Null(side.Sections);
    }

    [WindowsFact]
    public void WriteSide_RemovesThePreSplitValue()
    {
        using (var key = CreateKey($@"Layouts\{LayoutId}\PhysicalMonitors\{MonitorId}\BorderResistance"))
            key.SetValue("Left", "20", RegistryValueKind.String);

        WriteMonitor(new MonitorDto
        {
            BorderResistance = new BorderResistanceDto
            {
                Left = new BorderSideDto { Sections = [new BorderSectionDto { From = 0, To = 50, Move = 7 }] }
            }
        });

        // A registry key can hold a value AND a subkey of the same name at once, so
        // without the delete the legacy number would keep winning and the edge would be
        // frozen on its migrated setting, whatever the user edits.
        var side = ReadMonitor().BorderResistance!.Left!;
        Assert.Null(side.Move);
        Assert.Equal(7, Assert.Single(side.Sections!).Move);
    }

    [WindowsFact]
    public void WriteSide_RewritesTheSectionListWholesale()
    {
        var side = new BorderSideDto
        {
            Sections =
            [
                new BorderSectionDto { From = 0, To = 10 },
                new BorderSectionDto { From = 10, To = 20 },
                new BorderSectionDto { From = 20, To = 30 }
            ]
        };
        WriteMonitor(new MonitorDto { BorderResistance = new BorderResistanceDto { Bottom = side } });

        side.Sections = [new BorderSectionDto { From = 0, To = 5 }];
        WriteMonitor(new MonitorDto { BorderResistance = new BorderResistanceDto { Bottom = side } });

        // Leftover subkeys of a longer previous list would come back as phantom sections.
        var read = Assert.Single(ReadMonitor().BorderResistance!.Bottom!.Sections!);
        Assert.Equal(5, read.To);
    }

    [WindowsFact]
    public void ReadGlobalOptions_FallsBackToTheOptionsThatUsedToLiveInTheLayoutKey()
    {
        using (var key = CreateKey($@"Layouts\{LayoutId}"))
        {
            key.SetValue("HomeCinema", "1", RegistryValueKind.String);
            key.SetValue("StartMinimized", "1", RegistryValueKind.String);
            key.SetValue("Priority", "High", RegistryValueKind.String);
        }

        var read = Store.Read(LayoutId, []).GlobalOptions!;

        Assert.True(read.HomeCinema);
        Assert.True(read.StartMinimized);
        Assert.Equal("High", read.Priority);
    }

    [WindowsFact]
    public void ReadGlobalOptions_ReadsTheFormerShowAttachDetachWarningName()
    {
        using (var root = CreateKey("")) root.SetValue("ShowAttachDetachWarning", "0", RegistryValueKind.String);

        Assert.False(Store.Read("ANY", []).GlobalOptions!.ShowMonitorActionWarning);
    }

    [WindowsFact]
    public void WriteLayout_DeletesTheLegacyPriorityFromTheLayoutKey()
    {
        using (var key = CreateKey($@"Layouts\{LayoutId}"))
        {
            key.SetValue("Priority", "High", RegistryValueKind.String);
            key.SetValue("PriorityUnhooked", "Idle", RegistryValueKind.String);
        }

        // Still read before the first save, which is what keeps an upgrade from changing
        // anybody's priority...
        Assert.Equal("High", Store.Read(LayoutId, []).Layout!.Options!.Priority);

        Store.WriteLayout(LayoutId, new LayoutDto { Options = new LayoutOptionsDto { Enabled = true } });

        // ...and gone afterwards: Set() skips a null but never removes, so a value left
        // here would go on overriding the root key at every load, forever.
        using var layoutKey = Registry.CurrentUser.OpenSubKey($@"{_root}\Layouts\{LayoutId}")!;
        Assert.Null(layoutKey.GetValue("Priority"));
        Assert.Null(layoutKey.GetValue("PriorityUnhooked"));
        Assert.Equal("1", layoutKey.GetValue("Enabled"));
    }

    //==================//
    // Store contract   //
    //==================//

    [Fact] // a constant: no registry, so this one runs everywhere
    public void TheDefaultRootKey_IsTheHistoricalOne()
    {
        // The injected root exists for these tests only. Changing the real one orphans
        // every installation there has ever been, silently — they would start with
        // factory defaults and their layouts gone.
        Assert.Equal(@"SOFTWARE\Mgth\LittleBigMouse", RegistryLayoutStore.DefaultRootKey);
    }

    [WindowsFact]
    public void Read_NeverCreatesAnything()
    {
        // ILayoutStore: "Reads are PURE: they must never create keys nor seed values in
        // the store." The historical GetOrSet code did the exact opposite, and a read
        // that seeds is how a virtual layout ends up owning a key on this machine.
        var data = Store.Read("NOPE", [Pnp]);

        Assert.Null(data.GlobalOptions);
        Assert.Null(data.Layout);
        Assert.Empty(data.Models);
        Assert.Null(Registry.CurrentUser.OpenSubKey(_root));
    }

    [WindowsFact]
    public void WriteModels_UpsertsWithoutDroppingOthers()
    {
        Store.WriteModels(new Dictionary<string, ModelDto> { ["A"] = new() { Width = 1 } });
        Store.WriteModels(new Dictionary<string, ModelDto> { ["B"] = new() { Width = 2 } });

        var models = Store.Read("ANY", ["A", "B"]).Models;
        Assert.Equal(1, models["A"].Width);
        Assert.Equal(2, models["B"].Width);
    }

    [WindowsFact]
    public void Borders_AreOnlyReadBackWhenLeftIsThere()
    {
        // Presence of the Borders subkey is the "monitor owns its bezel borders" flag and
        // Left is what carries it: a partial subkey must not pass for ownership.
        WriteMonitor(new MonitorDto { Borders = new BordersDto { Top = 6 } });
        Assert.Null(ReadMonitor().Borders);

        WriteMonitor(new MonitorDto { Borders = new BordersDto { Left = 5 } });
        Assert.Equal(5, ReadMonitor().Borders!.Left);
    }

    [WindowsFact]
    public void Values_AreStoredAsTheHistoricalInvariantStrings()
    {
        // Byte-for-byte compatibility with every release that ever read these keys: all
        // values are REG_SZ, bools are "1"/"0", numbers are invariant. Under a
        // comma-decimal culture a plain ToString() writes "12,5", which older builds read
        // as zero — hence the culture switch here rather than trusting the CI locale.
        var previous = CultureInfo.CurrentCulture;
        CultureInfo.CurrentCulture = new CultureInfo("fr-FR");
        try
        {
            Store.WriteGlobalOptions(new GlobalOptionsDto
                { ExcludedDefaultsVersion = 3, Pinned = true, HomeCinema = false });
            WriteMonitor(new MonitorDto { XLocationInMm = 12.5 });

            using var root = Registry.CurrentUser.OpenSubKey(_root)!;
            Assert.Equal("3", root.GetValue("ExcludedDefaultsVersion"));
            Assert.Equal("1", root.GetValue("Pinned"));
            Assert.Equal("0", root.GetValue("HomeCinema"));
            Assert.Equal(RegistryValueKind.String, root.GetValueKind("Pinned"));

            using var monitor = Registry.CurrentUser.OpenSubKey(
                $@"{_root}\Layouts\{LayoutId}\PhysicalMonitors\{MonitorId}")!;
            Assert.Equal("12.5", monitor.GetValue("XLocationInMm"));
        }
        finally
        {
            CultureInfo.CurrentCulture = previous;
        }
    }

    //==================//
    // Helpers          //
    //==================//

    void WriteMonitor(MonitorDto monitor) => Store.WriteLayout(LayoutId, new LayoutDto
    {
        Options = new LayoutOptionsDto(),
        Monitors = { [MonitorId] = monitor }
    });

    MonitorDto ReadMonitor() => Store.Read(LayoutId, []).Layout!.Monitors[MonitorId];

    RegistryKey CreateKey(string relativePath) => Registry.CurrentUser.CreateSubKey(
        relativePath.Length == 0 ? _root : $@"{_root}\{relativePath}")!;

    /// <summary>
    /// One distinct, non-default value per property. Distinct matters as much as
    /// non-default: two fields wired to each other's registry name is the other mistake
    /// these parallel Set()/TryGet() lists invite, and identical values would hide it.
    /// A property type with no sample here fails loudly rather than being skipped.
    /// </summary>
    static T Filled<T>() where T : new()
    {
        var dto = new T();
        var i = 0;

        foreach (var property in typeof(T).GetProperties())
        {
            var type = Nullable.GetUnderlyingType(property.PropertyType) ?? property.PropertyType;

            object value =
                type == typeof(bool) ? i % 2 == 0 :
                type == typeof(int) ? 1000 + i :
                type == typeof(double) ? 100.5 + i :
                type == typeof(string) ? $"{property.Name}-{i}" :
                throw new NotSupportedException(
                    $"{typeof(T).Name}.{property.Name} is a {type.Name}: teach Filled<T> about it, "
                    + "or cover that property explicitly.");

            property.SetValue(dto, value);
            i++;
        }

        return dto;
    }

    /// <summary>Compare two DTOs property by property, naming the one that differs.</summary>
    static void AssertSameProperties(object expected, object actual, params string[] except)
    {
        foreach (var property in expected.GetType().GetProperties())
        {
            if (except.Contains(property.Name)) continue;

            Assert.Equal(
                $"{property.Name}={property.GetValue(expected)}",
                $"{property.Name}={property.GetValue(actual)}");
        }
    }
}
