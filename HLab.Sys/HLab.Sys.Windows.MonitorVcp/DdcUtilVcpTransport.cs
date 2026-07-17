#nullable enable
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Text.RegularExpressions;
using System.Threading;
using OneOf;

using static HLab.Sys.Windows.API.MonitorConfiguration.LowLevelMonitorConfiguration;

namespace HLab.Sys.Windows.MonitorVcp;

/// <summary>
/// DDC/CI through ddcutil on Linux, one instance per I2C bus. Reads are cached
/// for a short TTL and writes invalidate the cache: MonitorLevel polls its getter
/// in a tight loop, and every ddcutil call is a process spawn plus real traffic
/// on the DDC bus — without the cache the idle poll would hammer both.
/// </summary>
public sealed partial class DdcUtilVcpTransport(int bus) : IVcpTransport
{
    static readonly TimeSpan CacheTtl = TimeSpan.FromSeconds(1);

    // Throttle for polls answered from cache — keeps the CommandWorker loop
    // from spinning at 100% between two real reads.
    const int CacheHitSleepMs = 40;

    // Negative cache for the capabilities probe: on an unreachable monitor
    // ddcutil needs ~2s to give up, and VcpControl's probe retries — without
    // this each probe would pay that twice per monitor.
    static readonly TimeSpan CapsFailureTtl = TimeSpan.FromSeconds(10);

    readonly object _lock = new();
    readonly Dictionary<VcpCode, ((uint value, uint min, uint max) result, DateTime at)> _cache = new();
    DateTime _capsFailedAt = DateTime.MinValue;

    public int Bus => bus;

    public IReadOnlySet<byte>? GetSupportedCodes()
    {
        if (DateTime.UtcNow - _capsFailedAt < CapsFailureTtl) return null;

        var (ok, stdout) = Run($"capabilities --bus {bus}");
        if (!ok)
        {
            _capsFailedAt = DateTime.UtcNow;
            return null;
        }

        var codes = new HashSet<byte>();
        foreach (Match m in FeatureRegex().Matches(stdout))
            codes.Add(byte.Parse(m.Groups[1].Value, NumberStyles.HexNumber));

        return codes;
    }

    public OneOf<(uint value, uint min, uint max), int> GetFeature(VcpCode code)
    {
        lock (_lock)
        {
            if (_cache.TryGetValue(code, out var hit) && DateTime.UtcNow - hit.at < CacheTtl)
            {
                Thread.Sleep(CacheHitSleepMs);
                return hit.result;
            }

            var (ok, stdout) = Run($"getvcp {(byte)code:x2} --bus {bus} --brief");
            if (!ok) return -1;

            // Continuous:     "VCP 10 C 9 100"        (current, max)
            // Non-continuous: "VCP 60 SNC x0f"        (hex value)
            var tokens = stdout.Trim().Split(' ', StringSplitOptions.RemoveEmptyEntries);

            if (tokens is [_, _, "C", var cur, var max, ..]
                && uint.TryParse(cur, out var value) && uint.TryParse(max, out var maximum))
                return Cache(code, (value, 0u, maximum));

            if (tokens is [_, _, var type, .., var last] && type.EndsWith("NC") && TryParseHex(last, out var nc))
                return Cache(code, (nc, 0u, 0xFFu));

            return -1;
        }
    }

    public bool SetFeature(VcpCode code, uint value)
    {
        lock (_lock)
        {
            _cache.Remove(code);
            var (ok, _) = Run($"setvcp {(byte)code:x2} {value} --bus {bus}");
            return ok;
        }
    }

    (uint, uint, uint) Cache(VcpCode code, (uint, uint, uint) result)
    {
        _cache[code] = (result, DateTime.UtcNow);
        return result;
    }

    static bool TryParseHex(string token, out uint value)
    {
        value = 0;
        return token.Length > 1 && token[0] == 'x'
               && uint.TryParse(token[1..], NumberStyles.HexNumber, CultureInfo.InvariantCulture, out value);
    }

    static readonly bool PerfTrace =
        Environment.GetEnvironmentVariable("LBM_PERF") is "1" or "true" or "yes";

    static (bool ok, string stdout) Run(string args)
    {
        if (!PerfTrace) return RunCore(args);

        var sw = Stopwatch.StartNew();
        var result = RunCore(args);
        Console.Error.WriteLine(
            $"PERF {DateTime.Now:HH:mm:ss.fff} ddcutil {args} = {sw.ElapsedMilliseconds} ms ok={result.ok}");
        return result;
    }

    static (bool ok, string stdout) RunCore(string args)
    {
        try
        {
            using var process = Process.Start(new ProcessStartInfo("ddcutil", args)
            {
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
            });
            if (process is null) return (false, "");

            var stdout = process.StandardOutput.ReadToEnd();
            process.StandardError.ReadToEnd();

            if (!process.WaitForExit(10_000))
            {
                process.Kill(entireProcessTree: true);
                return (false, "");
            }

            return (process.ExitCode == 0, stdout);
        }
        catch (Exception)
        {
            return (false, "");
        }
    }

    public void Dispose() { }

    [GeneratedRegex(@"^\s*Feature:\s*([0-9A-Fa-f]{2})", RegexOptions.Multiline)]
    private static partial Regex FeatureRegex();
}
