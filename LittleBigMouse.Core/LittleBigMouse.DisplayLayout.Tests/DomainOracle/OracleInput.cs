using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using HLab.Sys.Monitors;
using LittleBigMouse.Platform.Linux;

namespace LittleBigMouse.DisplayLayout.Tests.DomainOracle;

/// <summary>
/// <c>domain-oracle/scenarios/&lt;name&gt;/input.json</c>: what the domain pipeline is given,
/// in a shape that owes nothing to the C# model — the outputs as the platform layer
/// enumerated them, and the configuration directory as it stood before the load.
/// <para>
/// Every member is <c>required</c> and unknown members are rejected: a corpus is read by
/// two languages, and a field one of them silently defaults is a field they may disagree on.
/// </para>
/// </summary>
sealed class OracleInput
{
    [JsonPropertyName("description")]
    public string? Description { get; init; }

    /// <summary>The enumerated outputs, in enumeration order (the order is an input too).</summary>
    [JsonPropertyName("displays")]
    public required List<OracleDisplay> Displays { get; init; }

    /// <summary>
    /// The JSON store directory before the load, keyed by path relative to it ("options.json",
    /// "models.json", "layouts/&lt;name&gt;.json"). A value is the file's JSON document; a
    /// JSON string stands for raw file text, for the files that are not valid JSON. Null or
    /// absent: nothing stored, a never-seen monitor set on a fresh install.
    /// </summary>
    [JsonPropertyName("store")]
    public Dictionary<string, JsonNode?>? Store { get; init; }

    /// <summary>
    /// The lines of the excluded-processes file (Excluded.txt, in the data directory rather
    /// than the store's) before the load. Null or absent: no such file yet.
    /// </summary>
    [JsonPropertyName("excluded")]
    public List<string>? Excluded { get; init; }

    public static readonly JsonSerializerOptions ReadOptions = new()
    {
        UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow,
    };

    public static OracleInput Parse(string json)
        => JsonSerializer.Deserialize<OracleInput>(json, ReadOptions)
           ?? throw new JsonException("input.json holds null");
}

/// <summary>
/// One enumerated output. Mirrors <see cref="LinuxMonitor"/> member for member, under the
/// same names — <c>DomainOracleTests.DisplayMirrorsLinuxMonitor</c> fails the day the record
/// gains or loses one, so the corpus format cannot fall behind the platform layer silently.
/// </summary>
sealed class OracleDisplay
{
    public required string ConnectorName { get; init; }

    public required double LogicalX { get; init; }
    public required double LogicalY { get; init; }
    public required double LogicalWidth { get; init; }
    public required double LogicalHeight { get; init; }

    public required int PixelWidth { get; init; }
    public required int PixelHeight { get; init; }

    public required double Scale { get; init; }

    public required double WidthMm { get; init; }
    public required double HeightMm { get; init; }

    public required bool Primary { get; init; }
    public required bool Enabled { get; init; }

    public required int Orientation { get; init; }

    public required int Frequency { get; init; }

    public required OracleEdid? Edid { get; init; }

    public LinuxMonitor ToLinuxMonitor() => new()
    {
        ConnectorName = ConnectorName,
        LogicalX = LogicalX,
        LogicalY = LogicalY,
        LogicalWidth = LogicalWidth,
        LogicalHeight = LogicalHeight,
        PixelWidth = PixelWidth,
        PixelHeight = PixelHeight,
        Scale = Scale,
        WidthMm = WidthMm,
        HeightMm = HeightMm,
        Primary = Primary,
        Enabled = Enabled,
        Orientation = Orientation,
        Frequency = Frequency,
        Edid = Edid?.ToEdid(),
    };
}

/// <summary>
/// The parsed EDID, reduced to the members <c>LinuxLayoutMapping.AddMonitor</c> reads — the
/// rest of <see cref="Edid"/> (gamma, chromaticity, DPMS…) never reaches the domain. Values
/// are what <see cref="EdidParser.Parse"/> produces: ProductCode is four uppercase hex digits,
/// Serial the eight hex digits of the binary serial (bytes 15..12), SerialNumber and Model
/// the 0xFF / 0xFC descriptor strings ("" when the EDID has none), PhysicalWidth/Height the
/// first detailed timing's image size in mm, VideoInterface the digital input name.
/// </summary>
sealed class OracleEdid
{
    public required string? ManufacturerCode { get; init; }
    public required string? ProductCode { get; init; }
    public required string? Serial { get; init; }
    public required string? SerialNumber { get; init; }
    public required string? Model { get; init; }
    public required double PhysicalWidth { get; init; }
    public required double PhysicalHeight { get; init; }
    public required string? VideoInterface { get; init; }

    public Edid ToEdid() => new()
    {
        ManufacturerCode = ManufacturerCode!,
        ProductCode = ProductCode!,
        Serial = Serial!,
        SerialNumber = SerialNumber!,
        Model = Model!,
        PhysicalWidth = PhysicalWidth,
        PhysicalHeight = PhysicalHeight,
        VideoInterface = VideoInterface!,
    };
}
