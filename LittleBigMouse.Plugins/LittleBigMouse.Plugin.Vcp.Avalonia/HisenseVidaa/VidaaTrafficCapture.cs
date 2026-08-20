#nullable enable
using System.Text;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

public sealed record VidaaTrafficMessage(DateTimeOffset Timestamp, string Topic, string Payload);

public static class VidaaTrafficCapture
{
    static readonly string[] SensitiveTopicParts =
    [
        "authentication",
        "tokenissuance",
        "gettoken",
        "refreshtoken",
        "authorization",
    ];

    public static bool IsSensitive(string topic, string payload)
        => SensitiveTopicParts.Any(part => topic.Contains(part, StringComparison.OrdinalIgnoreCase))
           || payload.Contains("accesstoken", StringComparison.OrdinalIgnoreCase)
           || payload.Contains("refreshtoken", StringComparison.OrdinalIgnoreCase);

    public static string DecodePayload(byte[] payload)
    {
        var text = Encoding.UTF8.GetString(payload).Trim();
        if (text.Length <= 2000) return text;
        return text[..2000] + "…";
    }

    public static string Format(IEnumerable<VidaaTrafficMessage> messages)
        => string.Join("\n\n", messages.Select(message =>
            $"[{message.Timestamp:HH:mm:ss.fff}] {message.Topic}\n{message.Payload}"));
}

/// <summary>
/// Keeps a traffic listener readable: retained replays, credentials, and the copies a device
/// publishes on several topics within the same instant never reach it.
/// </summary>
internal sealed class VidaaTrafficFilter(Action<VidaaTrafficMessage> onMessage)
{
    static readonly TimeSpan RepeatWindow = TimeSpan.FromMilliseconds(100);
    static readonly TimeSpan Staleness = TimeSpan.FromSeconds(5);

    readonly Dictionary<string, DateTimeOffset> _recentMessages = new(StringComparer.Ordinal);

    /// <summary>Called from the receive loop, hence the lock on the deduplication table.</summary>
    public void Handle(string topic, byte[] bytes, bool retained)
    {
        if (retained) return;
        var payload = VidaaTrafficCapture.DecodePayload(bytes);
        if (VidaaTrafficCapture.IsSensitive(topic, payload)) return;

        var now = DateTimeOffset.Now;
        var fingerprint = topic + "\n" + payload;
        lock (_recentMessages)
        {
            if (_recentMessages.TryGetValue(fingerprint, out var previous)
                && now - previous < RepeatWindow) return;
            _recentMessages[fingerprint] = now;
            if (_recentMessages.Count > 512)
                foreach (var stale in _recentMessages
                             .Where(pair => now - pair.Value > Staleness)
                             .Select(pair => pair.Key)
                             .ToArray())
                    _recentMessages.Remove(stale);
        }
        onMessage(new VidaaTrafficMessage(now, topic, payload));
    }
}
