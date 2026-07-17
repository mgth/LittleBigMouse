#nullable enable
using System.Text.Json;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;

public static class SamsungTizenProtocol
{
    public const string AppName = "LittleBigMouse";

    public static Uri RemoteUri(string ipAddress, string? token)
    {
        if (!SamsungTizenDevice.IsValidAddress(ipAddress))
            throw new ArgumentException("A valid IPv4 address is required.", nameof(ipAddress));

        var name = Uri.EscapeDataString(Convert.ToBase64String(System.Text.Encoding.UTF8.GetBytes(AppName)));
        var tokenQuery = string.IsNullOrWhiteSpace(token)
            ? ""
            : $"&token={Uri.EscapeDataString(token)}";

        return new Uri($"wss://{ipAddress}:8002/api/v2/channels/samsung.remote.control?name={name}{tokenQuery}");
    }

    public static string RemoteKeyPayload(string key, string command = "Click")
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(key);
        return JsonSerializer.Serialize(new
        {
            method = "ms.remote.control",
            @params = new
            {
                Cmd = command,
                DataOfCmd = key,
                Option = "false",
                TypeOfRemote = "SendRemoteKey",
            },
        });
    }

    /// <summary>
    /// Parses a user preset such as KEY_MENU,700,KEY_DOWN,KEY_ENTER. A number changes
    /// the delay after the preceding key; otherwise a conservative 150 ms is used.
    /// </summary>
    public static IReadOnlyList<(string Key, TimeSpan DelayAfter)> ParseSequence(string sequence)
        => RemoteMacro.Parse(sequence);

    public static SamsungTizenDevice ParseDevice(string ipAddress, string json)
    {
        using var document = JsonDocument.Parse(json);
        var root = document.RootElement;
        var device = root.TryGetProperty("device", out var nested) ? nested : root;

        string Read(params string[] names)
        {
            foreach (var name in names)
            {
                if (device.TryGetProperty(name, out var value) && value.ValueKind == JsonValueKind.String)
                    return value.GetString() ?? "";
            }
            return "";
        }

        var name = System.Net.WebUtility.HtmlDecode(Read("name"));
        if (string.IsNullOrWhiteSpace(name)
            && root.TryGetProperty("name", out var rootName)
            && rootName.ValueKind == JsonValueKind.String)
            name = System.Net.WebUtility.HtmlDecode(rootName.GetString() ?? "");

        var id = Read("duid", "id");
        if (string.IsNullOrWhiteSpace(id)
            && root.TryGetProperty("id", out var rootId)
            && rootId.ValueKind == JsonValueKind.String)
            id = rootId.GetString() ?? "";

        return new SamsungTizenDevice(
            ipAddress,
            string.IsNullOrWhiteSpace(name) ? "Samsung display" : name,
            Read("modelName", "model"),
            id,
            Read("wifiMac", "networkMac", "mac"));
    }

    /// <summary>Returns whether the TV accepted the client and the token issued with that event.</summary>
    public static (bool Connected, string Token, string Error) ParseChannelEvent(string json)
    {
        using var document = JsonDocument.Parse(json);
        var root = document.RootElement;
        var eventName = root.TryGetProperty("event", out var eventValue)
            ? eventValue.GetString() ?? ""
            : "";

        if (eventName is "ms.channel.unauthorized"
            or "ms.channel.clientDisconnect"
            or "ms.channel.timeOut")
            return (false, "", eventName);

        if (eventName != "ms.channel.connect") return (false, "", "");
        return (true, FindStringProperty(root, "token"), "");
    }

    static string FindStringProperty(JsonElement element, string name)
    {
        if (element.ValueKind == JsonValueKind.Object)
        {
            foreach (var property in element.EnumerateObject())
            {
                if (property.NameEquals(name) && property.Value.ValueKind == JsonValueKind.String)
                    return property.Value.GetString() ?? "";

                var nested = FindStringProperty(property.Value, name);
                if (!string.IsNullOrEmpty(nested)) return nested;
            }
        }
        else if (element.ValueKind == JsonValueKind.Array)
        {
            foreach (var item in element.EnumerateArray())
            {
                var nested = FindStringProperty(item, name);
                if (!string.IsNullOrEmpty(nested)) return nested;
            }
        }

        return "";
    }
}
