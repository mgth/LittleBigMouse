// EDID vectors for lbm-display: blocks of every length up to 256 bytes, random but
// shaped like EDIDs often enough to reach every branch of EdidParser.Parse (digital
// and analog inputs, every video interface, chromaticity, the four descriptor slots
// with names and serials), and what the C# parser makes of each. A block the parser
// throws on is recorded as such.
using System.Text.Json;
using System.Text.Json.Nodes;
using HLab.Sys.Monitors;

var random = new Random(20260911);
var vectors = new JsonArray();

byte[] Block(int length)
{
    var edid = new byte[length];
    random.NextBytes(edid);
    if (length > 20 && random.Next(2) == 0) edid[20] = (byte)(0x80 | random.Next(0x80));
    if (length > 23 && random.Next(4) == 0) edid[23] = 255;
    if (length > 24 && random.Next(2) == 0) edid[24] |= 0b100;
    // Display descriptors: 00 00 00 <tag> 00 <text, 0A, spaces>.
    for (var slot = 54; slot <= 108; slot += 18)
    {
        if (slot + 18 > length || random.Next(3) == 0) continue;
        byte tag = random.Next(3) switch { 0 => 0xFC, 1 => 0xFF, _ => (byte)random.Next(256) };
        edid[slot] = edid[slot + 1] = edid[slot + 2] = 0;
        edid[slot + 3] = tag;
        edid[slot + 4] = 0;
        var text = random.Next(14);
        for (var i = 0; i < 13; i++)
            edid[slot + 5 + i] = i < text ? (byte)random.Next(0x20, 0x100) : i == text ? (byte)0x0A : (byte)0x20;
    }
    return edid;
}

void Add(byte[] edid)
{
    var vector = new JsonObject { ["Edid"] = Convert.ToHexString(edid) };
    try
    {
        var e = EdidParser.Parse("KEY", edid);
        vector["Parsed"] = new JsonObject
        {
            ["HKeyName"] = e.HKeyName,
            ["ManufacturerCode"] = e.ManufacturerCode,
            ["ProductCode"] = e.ProductCode,
            ["Serial"] = e.Serial,
            ["Week"] = e.Week,
            ["Year"] = e.Year,
            ["Version"] = e.Version,
            ["Digital"] = e.Digital,
            ["BitDepth"] = e.BitDepth,
            ["VideoInterface"] = e.VideoInterface,
            ["PhysicalWidth"] = e.PhysicalWidth,
            ["PhysicalHeight"] = e.PhysicalHeight,
            ["Gamma"] = e.Gamma,
            ["DpmsStandbySupported"] = e.DpmsStandbySupported,
            ["DpmsSuspendSupported"] = e.DpmsSuspendSupported,
            ["DpmsActiveOffSupported"] = e.DpmsActiveOffSupported,
            ["YCrCb444Support"] = e.YCrCb444Support,
            ["YCrCb422Support"] = e.YCrCb422Support,
            ["RedX"] = e.RedX, ["RedY"] = e.RedY,
            ["GreenX"] = e.GreenX, ["GreenY"] = e.GreenY,
            ["BlueX"] = e.BlueX, ["BlueY"] = e.BlueY,
            ["WhiteX"] = e.WhiteX, ["WhiteY"] = e.WhiteY,
            ["Model"] = e.Model,
            ["SerialNumber"] = e.SerialNumber,
            ["Checksum"] = e.Checksum,
        };
    }
    catch (Exception ex)
    {
        vector["Throws"] = ex.GetType().Name;
    }
    vectors.Add(vector);
}

// Every length once, twice where a descriptor can run past the end (69..125, where the
// C# parser can throw), and the real sizes (one block, two) many times.
for (var length = 0; length <= 256; length++)
    for (var n = length is 128 or 256 ? 60 : length is >= 69 and <= 125 ? 2 : 1; n > 0; n--)
        Add(Block(length));

// One vector per line: small, and a regeneration diffs readably.
var json = "[\n" + string.Join(",\n", vectors.Select(v => v!.ToJsonString())) + "\n]\n";
File.WriteAllText(Path.Combine("..", "edid-vectors.json"), json);
Console.WriteLine($"{vectors.Count} vectors");
