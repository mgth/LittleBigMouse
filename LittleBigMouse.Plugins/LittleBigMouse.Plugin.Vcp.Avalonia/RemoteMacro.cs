#nullable enable

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

/// <summary>Shared parser for Samsung and VIDAA remote-key macros.</summary>
public static class RemoteMacro
{
    public static IReadOnlyList<(string Key, TimeSpan DelayAfter)> Parse(string sequence)
    {
        var result = new List<(string Key, TimeSpan DelayAfter)>();
        var tokens = sequence.Split(
            [',', ';', '+', '\n', '\r'],
            StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);

        foreach (var token in tokens)
        {
            if (int.TryParse(token, out var milliseconds))
            {
                if (result.Count == 0) throw new FormatException("A delay must follow a KEY_ command.");
                if (milliseconds is < 0 or > 10000)
                    throw new FormatException("Macro delays must be between 0 and 10000 ms.");
                result[^1] = (result[^1].Key, TimeSpan.FromMilliseconds(milliseconds));
                continue;
            }

            var key = token.ToUpperInvariant();
            if (!key.StartsWith("KEY_", StringComparison.Ordinal) || key.Any(char.IsWhiteSpace))
                throw new FormatException($"Invalid remote key: {token}");
            result.Add((key, TimeSpan.FromMilliseconds(150)));
        }

        if (result.Count == 0) throw new FormatException("Enter at least one KEY_ command.");
        return result;
    }
}
