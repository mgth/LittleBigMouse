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
