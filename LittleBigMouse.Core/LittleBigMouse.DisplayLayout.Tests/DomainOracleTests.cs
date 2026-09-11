using System.Reflection;
using LittleBigMouse.DisplayLayout.Tests.DomainOracle;
using LittleBigMouse.Platform.Linux;
using Xunit.Sdk;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The domain oracle: every scenario in <c>domain-oracle/scenarios</c> is run through the
/// real Linux pipeline and compared, file by file, with the outputs recorded next to it.
/// <para>
/// This exists for the Rust port. The C# domain core (DisplayLayout, Zoning, the persistence
/// engine) is being reimplemented, and the only way to know the port behaves is to compare it
/// against what this code does on inputs nobody can argue about — so the behaviour is written
/// down first, here, while the producer is still alive. The corpus is language-neutral on
/// purpose: JSON and XML in, JSON and XML out, no C# type in sight.
/// </para>
/// <para>
/// Authority: as long as this test runs, the C# code is the producer of record. A Rust
/// mismatch is a Rust bug unless the scenario's description says the deviation is deliberate.
/// See <c>domain-oracle/README.md</c> — including for what a failure here means (a behaviour
/// change, wanted or not) and how to regenerate with <c>LBM_UPDATE_GOLDEN=1</c>.
/// </para>
/// </summary>
public class DomainOracleTests
{
    //==================//
    // Corpus IO        //
    //==================//

    /// <summary>
    /// The corpus in the SOURCE tree, not in a build-output copy — the same rule as
    /// <see cref="WireContractGoldenTests"/>, and for the same reason: the Rust port reads
    /// these very bytes, and a copy in bin/ would let the two drift apart silently.
    /// </summary>
    static string CorpusDir
    {
        get
        {
            var dir = new DirectoryInfo(AppContext.BaseDirectory);
            while (dir is not null)
            {
                var candidate = Path.Combine(dir.FullName, "domain-oracle");
                if (Directory.Exists(candidate)) return candidate;
                dir = dir.Parent;
            }
            throw new DirectoryNotFoundException("domain-oracle not found above " + AppContext.BaseDirectory);
        }
    }

    static string ScenariosDir => Path.Combine(CorpusDir, "scenarios");

    public static TheoryData<string> Scenarios
    {
        get
        {
            var data = new TheoryData<string>();
            foreach (var dir in Directory.GetDirectories(ScenariosDir).Select(Path.GetFileName).Order(StringComparer.Ordinal))
                data.Add(dir!);
            return data;
        }
    }

    static bool Updating => Environment.GetEnvironmentVariable("LBM_UPDATE_GOLDEN") == "1";

    /// <summary>A checked-in file as text, whatever the checkout did to its line endings.</summary>
    static string Read(string path) => File.ReadAllText(path).Replace("\r\n", "\n");

    //==================//
    // The oracle       //
    //==================//

    [Theory]
    [MemberData(nameof(Scenarios))]
    public void ScenarioReproducesItsRecordedOutputs(string scenario)
    {
        var dir = Path.Combine(ScenariosDir, scenario);
        var expectedDir = Path.Combine(dir, "expected");

        var outputs = OracleRun.Outputs(OracleInput.Parse(Read(Path.Combine(dir, "input.json"))));

        if (Updating)
        {
            Directory.CreateDirectory(expectedDir);
            foreach (var stale in Files(expectedDir).Where(f => !outputs.ContainsKey(f)))
                File.Delete(Path.Combine(expectedDir, stale));

            foreach (var (name, content) in outputs)
                File.WriteAllText(Path.Combine(expectedDir, name), content);
        }

        Assert.True(Directory.Exists(expectedDir),
            $"{scenario} has no expected/ directory; LBM_UPDATE_GOLDEN=1 records one.");

        // The file SET is part of the contract: a scenario silently losing its zones.xml
        // would otherwise pass.
        Assert.Equal(outputs.Keys, Files(expectedDir));

        foreach (var (name, actual) in outputs)
        {
            var expected = Read(Path.Combine(expectedDir, name));
            if (expected == actual) continue;

            try
            {
                Assert.Equal(expected, actual);
            }
            catch (EqualException error)
            {
                throw new XunitException(
                    $"scenarios/{scenario}/expected/{name} is not what the domain produces any more. "
                    + "If the change is intended, regenerate with LBM_UPDATE_GOLDEN=1 and read the "
                    + $"diff before committing it.\n{error.Message}");
            }
        }
    }

    static IReadOnlyList<string> Files(string dir) =>
        [.. Directory.GetFiles(dir, "*", SearchOption.AllDirectories)
            .Select(f => Path.GetRelativePath(dir, f).Replace(Path.DirectorySeparatorChar, '/'))
            .Order(StringComparer.Ordinal)];

    //==================//
    // Corpus hygiene   //
    //==================//

    [Fact]
    public void CorpusHoldsScenarios()
    {
        // A resolution that silently found nothing would make every theory above vacuous.
        var count = Directory.GetDirectories(ScenariosDir).Length;
        Assert.True(count >= 15, $"only {count} scenarios found in {ScenariosDir}");
    }

    [Fact]
    public void ReadmeListsEveryScenario()
    {
        // The list of scenarios is the README's table of contents: a scenario nobody can
        // find is a scenario nobody maintains.
        var readme = Read(Path.Combine(CorpusDir, "README.md"));

        foreach (var scenario in Directory.GetDirectories(ScenariosDir).Select(Path.GetFileName))
            Assert.True(readme.Contains($"`{scenario}`", StringComparison.Ordinal),
                $"domain-oracle/README.md does not mention the scenario `{scenario}`");
    }

    /// <summary>
    /// The corpus describes an output with the platform layer's own field set. A field added
    /// to <see cref="LinuxMonitor"/> and not to the corpus would reach the domain with a
    /// default value nobody wrote down — this is what makes that a compile-and-test failure
    /// rather than a silent hole.
    /// </summary>
    [Fact]
    public void DisplayMirrorsLinuxMonitor()
    {
        Assert.Equal(Members(typeof(LinuxMonitor)), Members(typeof(OracleDisplay)));

        static IEnumerable<string> Members(Type type) =>
            type.GetProperties(BindingFlags.Public | BindingFlags.Instance)
                // The compiler-generated member of a record type, not a field of the model.
                .Where(p => p.Name != "EqualityContract")
                .Select(p => p.Name)
                .Order(StringComparer.Ordinal);
    }
}
