using LittleBigMouse.Platform.Linux;
using LittleBigMouse.Plugins.Persistence;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// Locks the on-disk JSON format of the Linux backend: the literal documents here are the
/// files written by the pre-refactor <c>LinuxLayoutPersistence</c>, so a DTO rename or a
/// serializer change that would orphan existing user configs fails these tests.
/// </summary>
public class JsonLayoutStoreTests : IDisposable
{
    readonly string _dir = Path.Combine(Path.GetTempPath(), "lbm-json-store-tests", Guid.NewGuid().ToString("N"));

    public void Dispose()
    {
        try { Directory.Delete(_dir, true); } catch { }
    }

    [Fact]
    public void Read_CurrentOnDiskFormat()
    {
        Directory.CreateDirectory(Path.Combine(_dir, "layouts"));
        File.WriteAllText(Path.Combine(_dir, "options.json"), """
        {
          "DaemonPort": 25196,
          "Priority": "Normal",
          "HomeCinema": false,
          "ShowMonitorActionWarning": true,
          "BorderValues": "PerModel"
        }
        """);
        File.WriteAllText(Path.Combine(_dir, "models.json"), """
        {
          "TST1234": {
            "Width": 600,
            "Height": 340,
            "Borders": { "Left": 10, "Top": 11, "Right": 12, "Bottom": 13 },
            "PnpName": "Test monitor"
          }
        }
        """);
        File.WriteAllText(Path.Combine(_dir, "layouts", "LAYOUT1.json"), """
        {
          "Options": { "Enabled": true, "Algorithm": "Strait", "MaxTravelDistance": 200 },
          "Monitors": {
            "MON1": {
              "XLocationInMm": 12.5,
              "YLocationInMm": -3,
              "PhysicalRatioX": 1,
              "BorderResistance": { "Left": 1, "Top": 2, "Right": 3, "Bottom": 4 },
              "Borders": { "Left": 5, "Top": 6, "Right": 7, "Bottom": 8 },
              "ActiveSource": "SRC1",
              "ExcludedFromLayout": false,
              "Sources": {
                "SRC1": { "PixelX": 0, "PixelY": 0, "PixelWidth": 1920, "PixelHeight": 1080, "Orientation": 0, "Primary": true }
              }
            }
          }
        }
        """);

        var store = new JsonLayoutStore(_dir);
        var data = store.Read("LAYOUT1", ["TST1234"]);

        Assert.Equal(25196, data.GlobalOptions!.DaemonPort);
        Assert.Equal("Normal", data.GlobalOptions.Priority);
        Assert.True(data.GlobalOptions.ShowMonitorActionWarning);

        Assert.Equal(600, data.Models["TST1234"].Width);
        Assert.Equal(13, data.Models["TST1234"].Borders!.Bottom);
        Assert.Equal("Test monitor", data.Models["TST1234"].PnpName);

        var layout = data.Layout!;
        Assert.True(layout.Options!.Enabled);
        Assert.Equal(200, layout.Options.MaxTravelDistance);

        var monitor = layout.Monitors["MON1"];
        Assert.Equal(12.5, monitor.XLocationInMm);
        Assert.Equal(4, monitor.BorderResistance!.Bottom);
        Assert.Equal(5, monitor.Borders!.Left);
        Assert.Equal("SRC1", monitor.ActiveSource);
        Assert.True(monitor.Sources!["SRC1"].Primary);
        Assert.Equal(1920, monitor.Sources["SRC1"].PixelWidth);
    }

    [Fact]
    public void WriteThenRead_RoundTrips()
    {
        var store = new JsonLayoutStore(_dir);

        store.WriteGlobalOptions(new GlobalOptionsDto { Pinned = true, ExcludedDefaultsVersion = 1 });
        store.WriteLayout("L1", new LayoutDto
        {
            Options = new LayoutOptionsDto { Enabled = true },
            Monitors = { ["M1"] = new MonitorDto { XLocationInMm = 7.25 } }
        });

        var data = store.Read("L1", []);

        Assert.True(data.GlobalOptions!.Pinned);
        Assert.Equal(1, data.GlobalOptions.ExcludedDefaultsVersion);
        Assert.True(data.Layout!.Options!.Enabled);
        Assert.Equal(7.25, data.Layout.Monitors["M1"].XLocationInMm);
    }

    [Fact]
    public void WriteModels_UpsertsWithoutDroppingOthers()
    {
        var store = new JsonLayoutStore(_dir);

        store.WriteModels(new Dictionary<string, ModelDto> { ["A"] = new() { Width = 1 } });
        store.WriteModels(new Dictionary<string, ModelDto> { ["B"] = new() { Width = 2 } });

        var data = store.Read("x", ["A", "B"]);
        Assert.Equal(1, data.Models["A"].Width);
        Assert.Equal(2, data.Models["B"].Width);
    }

    [Fact]
    public void Read_CorruptFile_FallsBackToDefaults()
    {
        Directory.CreateDirectory(_dir);
        File.WriteAllText(Path.Combine(_dir, "options.json"), "{ this is not json");

        var store = new JsonLayoutStore(_dir);
        Assert.Null(store.Read("x", []).GlobalOptions);
    }

    [Fact]
    public void LayoutId_IsSanitizedForTheFileSystem()
    {
        var store = new JsonLayoutStore(_dir);
        store.WriteLayout("A/B", new LayoutDto { Options = new LayoutOptionsDto { Enabled = true } });

        Assert.True(File.Exists(Path.Combine(_dir, "layouts", "A_B.json")));
        Assert.True(store.Read("A/B", []).Layout!.Options!.Enabled);
    }
}
