using System.Globalization;
using System.Text.Json;

// Pairs of strings and how .NET's StringComparer.InvariantCulture orders them,
// the comparer the LittleBigMouse domain sorts monitor ids and source device ids
// with (OrderBy(s => s) / SortExpressionComparer under the pinned invariant culture).
CultureInfo.CurrentCulture = CultureInfo.InvariantCulture;
var rng = new Random(1234);
var strings = new List<string>();
// Every printable ASCII char alone and paired with letters/digits, to pin weights.
for (var c = 0x20; c < 0x7f; c++) { strings.Add(((char)c).ToString()); strings.Add("a" + (char)c + "b"); strings.Add((char)c + "1"); }
// Realistic ids.
string[] connectors = ["DP-1","DP-2","DP-3","DP-10","DP-0","HDMI-A-1","HDMI-A-2","HDMI-0","eDP-1","eDP-2","DVI-D-1","VGA-1","FALLBACK","dp-1","Virtual-1","DisplayPort-0","USB-C-1"];
strings.AddRange(connectors);
string[] pnps = ["DELD0D9","DEL42E2","DELD177","DELA0BE","PHL0927","SAME035","HEC002F","HEC0030","GSM5B7F","ACR0620","AUO123D","BOE0A1B","NOEDID"];
foreach (var p in pnps)
{
    strings.Add(p);
    strings.Add($"{p}_FAKE24000001");
    strings.Add($"{p}_00000001");
    strings.Add($"{p}_H1AK500000");
    strings.Add($"{p}_H1AK500000_1");
    strings.Add($"{p}_00000001@DP-2");
    strings.Add($"{p}_00000001@HDMI-A-1");
    strings.Add($"{p}P2PC251R08GL_1A_07E4_3F");
    strings.Add($"NOEDID_{p}_5&1a2b3c&0&UID4352");
}
// Random strings over the id alphabet.
const string alphabet = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-@.+:#&\\/ ";
for (var i = 0; i < 4000; i++)
{
    var len = rng.Next(1, 9);
    var chars = new char[len];
    for (var j = 0; j < len; j++) chars[j] = alphabet[rng.Next(alphabet.Length)];
    strings.Add(new string(chars));
}
var cmp = StringComparer.InvariantCulture;
var fixedCount = strings.Count - 4000;
var sorted = strings.Take(fixedCount).Distinct().ToList();
sorted.Sort(cmp);
var pairs = new List<object>();
for (var k = 0; k < 3000; k++)
{
    var a = strings[rng.Next(strings.Count)];
    var b = strings[rng.Next(strings.Count)];
    pairs.Add(new object[] { a, b, Math.Sign(cmp.Compare(a, b)) });
}
Console.WriteLine(JsonSerializer.Serialize(
    new { runtime = System.Runtime.InteropServices.RuntimeInformation.FrameworkDescription, sorted, pairs },
    new JsonSerializerOptions { WriteIndented = false, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping }));
