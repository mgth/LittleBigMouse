#nullable enable
using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Xml.Linq;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

public enum VidaaAuthMethod
{
    Legacy,
    Middle,
    Modern,
}

public sealed record VidaaMqttCredentials(string ClientId, string Username, string Password);
public sealed record VidaaPictureSetting(
    int MenuId,
    string Name,
    string Value,
    string ValueType,
    int? Flag);

public static class HisenseVidaaProtocol
{
    public const int MqttPort = 36669;
    public const int SsdpPort = 1900;
    public const string SsdpAddress = "239.255.255.250";
    public const string DynamicPattern = "38D65DC30F45109A369A86FCE866A85B";
    public const string ModernSuffix = "h!i@s#$v%i^d&a*a";
    public const string LegacySuffix = "h*i&s%e!r^v0i1c9";
    public const ulong TimeXorConstant = 0x5698_1477_2b03_a968;
    public const string LegacyMqttUsername = "hisenseservice";
    public const string LegacyMqttPassword = "multimqttservice";

    // The C1 answers on 18400 while leaving 38400 unanswered. Trying the live
    // port first also lets us learn transport_protocol before MQTT auth.
    public static readonly int[] DescriptorPorts = [18400, 38400];

    public static VidaaAuthMethod AuthMethodFor(int? protocolVersion) => protocolVersion switch
    {
        >= 3290 => VidaaAuthMethod.Modern,
        >= 3000 => VidaaAuthMethod.Middle,
        null => VidaaAuthMethod.Modern,
        _ => VidaaAuthMethod.Legacy,
    };

    public static bool UsesStaticLegacyProtocol(int? protocolVersion)
        => protocolVersion is > 0 and < 3000;

    public static string LegacyDeviceTopic(string controllerMac)
        => $"{NormalizeMac(controllerMac).ToUpperInvariant()}$normal";

    public static VidaaMqttCredentials GenerateCredentials(
        string macAddress,
        VidaaAuthMethod authMethod,
        long? unixTimeSeconds = null,
        string brand = "his")
    {
        var uuid = NormalizeMac(macAddress, preserveCase: true);
        if (string.IsNullOrWhiteSpace(uuid))
            throw new ArgumentException("A Wi-Fi MAC address is required for VIDAA authentication.", nameof(macAddress));

        var timestamp = unixTimeSeconds ?? DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        var raceHash = Md5($"{DynamicPattern}${uuid}")[..6];
        var clientId = $"{uuid}${brand}${raceHash}_vidaacommon_001";
        var usernameTime = authMethod == VidaaAuthMethod.Legacy
            ? unchecked((ulong)timestamp)
            : unchecked((ulong)timestamp) ^ TimeXorConstant;
        var username = $"{brand}${usernameTime}";
        var remainder = timestamp.ToString(CultureInfo.InvariantCulture)
            .Where(char.IsAsciiDigit)
            .Sum(c => c - '0') % 10;
        var suffix = authMethod == VidaaAuthMethod.Modern ? ModernSuffix : LegacySuffix;
        var valueHash = Md5($"{brand}{remainder}{suffix}")[..6];
        var password = Md5($"{timestamp}${valueHash}");
        return new VidaaMqttCredentials(clientId, username, password);
    }

    public static HisenseVidaaDevice ParseDescriptor(string ipAddress, string xml)
    {
        if (!HisenseVidaaDevice.IsValidAddress(ipAddress))
            throw new ArgumentException("Enter a valid IPv4 address.", nameof(ipAddress));

        var document = XDocument.Parse(xml);
        string Element(string name) => document.Descendants()
            .FirstOrDefault(e => e.Name.LocalName.Equals(name, StringComparison.OrdinalIgnoreCase))
            ?.Value.Trim() ?? "";

        var raw = ParseDescription(Element("modelDescription"));
        if (raw.TryGetValue("vidaa_support", out var support) && support == "0")
            throw new InvalidDataException("The network device does not advertise VIDAA remote control.");

        var mac = First(raw, "mac", "macWifi", "macEthernet");
        var protocol = int.TryParse(First(raw, "transport_protocol"), out var version) ? (int?)version : null;
        var model = Element("modelName");
        if (model.Equals("Renderer", StringComparison.OrdinalIgnoreCase)) model = "";
        var name = Element("friendlyName");
        return new HisenseVidaaDevice(
            ipAddress,
            string.IsNullOrWhiteSpace(name) ? "Hisense VIDAA" : name,
            model,
            NormalizeMac(mac),
            protocol,
            First(raw, "brand") is { Length: > 0 } brand ? brand : "his");
    }

    public static string TranslateKey(string key) => key.Trim().ToUpperInvariant() switch
    {
        "KEY_ENTER" => "KEY_OK",
        "KEY_RETURN" => "KEY_RETURNS",
        "KEY_VOLUP" => "KEY_VOLUMEUP",
        "KEY_VOLDOWN" => "KEY_VOLUMEDOWN",
        "KEY_CHUP" => "KEY_CHANNELUP",
        "KEY_CHDOWN" => "KEY_CHANNELDOWN",
        "KEY_PLAYPAUSE" => "KEY_PLAY",
        "KEY_FF" => "KEY_FORWARDS",
        "KEY_REWIND" => "KEY_BACKWARDS",
        var value when value.StartsWith("KEY_", StringComparison.Ordinal) => value,
        _ => throw new ArgumentException("Invalid VIDAA remote key.", nameof(key)),
    };

    public static string Topic(string service, string clientId, string action)
        => $"/remoteapp/tv/{service}/{clientId}/actions/{action}";

    public static string PictureSettingPayload(int menuId, int value)
    {
        if (menuId is < 1 or > 999)
            throw new ArgumentOutOfRangeException(nameof(menuId), "Enter a VIDAA picture-setting menu ID between 1 and 999.");
        if (value is < -1000 or > 1000)
            throw new ArgumentOutOfRangeException(nameof(value), "Enter a VIDAA picture-setting value between -1000 and 1000.");
        return JsonSerializer.Serialize(new
        {
            action = "set_value",
            menu_id = menuId,
            menu_value = value,
            menu_value_type = "int",
        });
    }

    public static string PictureSettingsRequestPayload()
        => "{\"action\":\"get_menu_info\"}";

    public static bool TryParsePictureSettings(
        string topic,
        string payload,
        out IReadOnlyList<VidaaPictureSetting> settings)
    {
        settings = [];
        if (!topic.EndsWith("/platform_service/data/picturesetting", StringComparison.OrdinalIgnoreCase))
            return false;
        try
        {
            using var json = JsonDocument.Parse(payload);
            var result = new List<VidaaPictureSetting>();
            var root = json.RootElement;
            if (root.ValueKind == JsonValueKind.Object
                && root.TryGetProperty("menu_info", out var menuInfo)
                && menuInfo.ValueKind == JsonValueKind.Array)
            {
                foreach (var item in menuInfo.EnumerateArray())
                    if (ParsePictureSetting(item) is { } setting) result.Add(setting);
            }
            else if (root.ValueKind == JsonValueKind.Array)
            {
                foreach (var item in root.EnumerateArray())
                    if (ParsePictureSetting(item) is { } setting) result.Add(setting);
            }
            else if (ParsePictureSetting(root) is { } setting)
            {
                result.Add(setting);
            }
            settings = result;
            return result.Count > 0;
        }
        catch (JsonException)
        {
            return false;
        }
    }

    public static string VolumePayload(int volume)
    {
        if (volume is < 0 or > 100)
            throw new ArgumentOutOfRangeException(nameof(volume), "Enter a volume between 0 and 100.");
        return volume.ToString(CultureInfo.InvariantCulture);
    }

    public static string PlatformActionName(string action)
    {
        var normalized = action.Trim();
        if (normalized.Length is 0 or > 64
            || normalized.Any(c => !char.IsAsciiLetterOrDigit(c) && c is not '_' and not '-'))
            throw new ArgumentException(
                "Enter a platform action name containing only letters, digits, '_' or '-'.",
                nameof(action));
        return normalized;
    }

    public static string ExperimentalLevelPayload(int value)
    {
        if (value is < 0 or > 10)
            throw new ArgumentOutOfRangeException(nameof(value), "Enter a test level between 0 and 10.");
        return value.ToString(CultureInfo.InvariantCulture);
    }

    public static bool TryParseVolume(string topic, string payload, out int volume)
    {
        volume = 0;
        if (!topic.EndsWith("/platform_service/actions/volumechange", StringComparison.OrdinalIgnoreCase))
            return false;
        try
        {
            using var json = JsonDocument.Parse(payload);
            return json.RootElement.TryGetProperty("volume_value", out var value)
                   && value.TryGetInt32(out volume)
                   && volume is >= 0 and <= 100;
        }
        catch (JsonException)
        {
            return false;
        }
    }

    public static IReadOnlyList<string> ResponseTopics(string clientId) =>
    [
        $"/remoteapp/mobile/{clientId}/ui_service/data/authentication",
        $"/remoteapp/mobile/{clientId}/ui_service/data/authenticationcode",
        $"/remoteapp/mobile/{clientId}/ui_service/data/authenticationcodetoast",
        $"/remoteapp/mobile/{clientId}/ui_service/data/authenticationcodeclose",
        $"/remoteapp/mobile/{clientId}/ui_service/data/tokenissuance",
        $"/remoteapp/mobile/{clientId}/platform_service/data/tokenissuance",
        "/remoteapp/mobile/broadcast/ui_service/state",
        "/remoteapp/mobile/broadcast/platform_service/actions/volumechange",
        "/remoteapp/mobile/broadcast/platform_service/data/picturesetting",
    ];

    public static IReadOnlyList<string> LegacyResponseTopics(string deviceTopic) =>
    [
        $"/remoteapp/mobile/{deviceTopic}/#",
        "/remoteapp/mobile/broadcast/#",
    ];

    public static string PairingRequestPayload() => JsonSerializer.Serialize(new
    {
        app_version = 2,
        connect_result = 0,
        device_type = "Mobile App",
    });

    public static string PinPayload(string pin)
    {
        if (pin.Length != 4 || !pin.All(char.IsAsciiDigit))
            throw new ArgumentException("Enter the four-digit PIN displayed by the Hisense device.", nameof(pin));
        return JsonSerializer.Serialize(new { authNum = int.Parse(pin, CultureInfo.InvariantCulture) });
    }

    public static string LegacyPinPayload(string pin)
    {
        if (pin.Length != 4 || !pin.All(char.IsAsciiDigit))
            throw new ArgumentException("Enter the four-digit PIN displayed by the Hisense device.", nameof(pin));
        return JsonSerializer.Serialize(new { authNum = pin });
    }

    public static string NormalizeMac(string? value, bool preserveCase = false)
    {
        if (string.IsNullOrWhiteSpace(value)) return "";
        var compact = new string(value.Where(Uri.IsHexDigit).ToArray());
        if (compact.Length != 12) return value.Trim();
        var result = string.Join(':', Enumerable.Range(0, 6).Select(i => compact.Substring(i * 2, 2)));
        return preserveCase ? result : result.ToUpperInvariant();
    }

    static Dictionary<string, string> ParseDescription(string value)
    {
        var result = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (var part in value.Split(['\r', '\n', ';'], StringSplitOptions.RemoveEmptyEntries))
        {
            var separator = part.IndexOf('=');
            if (separator <= 0) continue;
            result[part[..separator].Trim()] = part[(separator + 1)..].Trim();
        }
        return result;
    }

    static VidaaPictureSetting? ParsePictureSetting(JsonElement item)
    {
        if (item.ValueKind != JsonValueKind.Object
            || !item.TryGetProperty("menu_id", out var menuId)
            || !menuId.TryGetInt32(out var id)) return null;

        var name = item.TryGetProperty("menu_name", out var menuName)
            ? menuName.GetString() ?? ""
            : "";
        var value = item.TryGetProperty("menu_value", out var menuValue)
            ? menuValue.ValueKind == JsonValueKind.String
                ? menuValue.GetString() ?? ""
                : menuValue.GetRawText()
            : "";
        var valueType = item.TryGetProperty("menu_value_type", out var type)
            ? type.GetString() ?? ""
            : "";
        int? flag = item.TryGetProperty("menu_flag", out var menuFlag)
                    && menuFlag.TryGetInt32(out var parsedFlag)
            ? parsedFlag
            : null;
        return new VidaaPictureSetting(id, name, value, valueType, flag);
    }

    static string First(Dictionary<string, string> values, params string[] keys)
    {
        foreach (var key in keys)
            if (values.TryGetValue(key, out var value) && !string.IsNullOrWhiteSpace(value)) return value;
        return "";
    }

    static string Md5(string value)
        => Convert.ToHexString(MD5.HashData(Encoding.UTF8.GetBytes(value)));
}
