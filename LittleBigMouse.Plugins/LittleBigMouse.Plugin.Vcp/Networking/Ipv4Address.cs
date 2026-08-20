using System.Diagnostics.CodeAnalysis;
using System.Net;
using System.Net.Sockets;

namespace LittleBigMouse.Plugin.Vcp.Networking;

/// <summary>
/// The single rule every network-attached display in this plugin agrees on: it is reached at a
/// literal IPv4 address. A host name is refused on purpose — the address is also what the
/// discovery answers carry, what the pairing token is bound to and what a Wake-on-LAN burst is
/// aimed at, so accepting something that needs resolving would move a failure from the setup
/// panel to the middle of a command.
///
/// Surrounding blanks are accepted and stripped: every one of these addresses arrives from a
/// text box.
/// </summary>
public static class Ipv4Address
{
    /// <summary>The message shown when an address entered by hand cannot be used.</summary>
    public const string InvalidMessage = "Enter a valid IPv4 address.";

    public static bool IsValid([NotNullWhen(true)] string? value) => TryParse(value, out _);

    public static bool TryParse(string? value, [NotNullWhen(true)] out IPAddress? address)
    {
        address = null;
        if (string.IsNullOrWhiteSpace(value)) return false;
        if (!IPAddress.TryParse(value.Trim(), out var parsed)
            || parsed.AddressFamily != AddressFamily.InterNetwork) return false;

        address = parsed;
        return true;
    }

    /// <summary>
    /// Returns the address without its surrounding blanks, so callers store and send the same
    /// text they validated.
    /// </summary>
    /// <exception cref="ArgumentException">The value is not a literal IPv4 address.</exception>
    public static string Require(string? value, string parameterName, string? message = null)
        => IsValid(value)
            ? value.Trim()
            : throw new ArgumentException(message ?? InvalidMessage, parameterName);
}
