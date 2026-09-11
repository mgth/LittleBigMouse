using LittleBigMouse.DisplayLayout.Tests.DomainOracle;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// Runs <see cref="DisplayDump"/> on demand: with <c>LBM_DUMP_DISPLAYS</c> set to a file
/// path, the discovery of the machine running the suite is written there, to be compared
/// with <c>lbm-agent --dump-displays</c>; without it, nothing happens (CI machines have no
/// displays worth dumping).
/// <code>
/// LBM_DUMP_DISPLAYS=/tmp/cs.json dotnet test LittleBigMouse.Core/LittleBigMouse.DisplayLayout.Tests --filter DumpDisplays
/// </code>
/// </summary>
public class DisplayDumpTests
{
    [Fact]
    public void DumpDisplays()
    {
        var path = Environment.GetEnvironmentVariable("LBM_DUMP_DISPLAYS");
        if (string.IsNullOrEmpty(path)) return;
        if (!OperatingSystem.IsLinux()) return; // the Windows twin comes with the Windows discovery

        File.WriteAllText(path, DisplayDump.Linux());
    }
}
