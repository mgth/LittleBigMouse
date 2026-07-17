#nullable enable
using System.Net;
using System.Net.Http;
using System.Net.Sockets;
using System.Text;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;

public sealed class SamsungTizenDiscovery(HttpClient httpClient)
{
    static readonly IPEndPoint SsdpEndpoint = new(IPAddress.Parse("239.255.255.250"), 1900);

    public async Task<SamsungTizenDevice> ProbeAsync(
        string ipAddress,
        CancellationToken cancellationToken = default)
    {
        if (!SamsungTizenDevice.IsValidAddress(ipAddress))
            throw new ArgumentException("Enter a valid IPv4 address.", nameof(ipAddress));

        using var response = await httpClient.GetAsync(
            $"http://{ipAddress}:8001/api/v2/", cancellationToken).ConfigureAwait(false);
        response.EnsureSuccessStatusCode();
        var json = await response.Content.ReadAsStringAsync(cancellationToken).ConfigureAwait(false);
        return SamsungTizenProtocol.ParseDevice(ipAddress, json);
    }

    public async Task<IReadOnlyList<SamsungTizenDevice>> DiscoverAsync(
        TimeSpan timeout,
        CancellationToken cancellationToken = default)
    {
        using var udp = new UdpClient(AddressFamily.InterNetwork);
        foreach (var searchTarget in new[] { "urn:samsung.com:device:RemoteControlReceiver:1", "ssdp:all" })
        {
            var request = Encoding.ASCII.GetBytes(
                "M-SEARCH * HTTP/1.1\r\n" +
                "HOST: 239.255.255.250:1900\r\n" +
                "MAN: \"ssdp:discover\"\r\n" +
                "MX: 2\r\n" +
                $"ST: {searchTarget}\r\n\r\n");
            await udp.SendAsync(request, SsdpEndpoint, cancellationToken).ConfigureAwait(false);
        }

        using var deadline = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        deadline.CancelAfter(timeout);
        var addresses = new HashSet<string>(StringComparer.Ordinal);

        try
        {
            while (true)
            {
                var response = await udp.ReceiveAsync(deadline.Token).ConfigureAwait(false);
                var text = Encoding.UTF8.GetString(response.Buffer);
                var location = ParseHeader(text, "LOCATION");
                if (Uri.TryCreate(location, UriKind.Absolute, out var uri)
                    && IPAddress.TryParse(uri.Host, out _))
                    addresses.Add(uri.Host);
                else
                    addresses.Add(response.RemoteEndPoint.Address.ToString());
            }
        }
        catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
        {
            // Normal end of the discovery window.
        }

        var probes = addresses.Select(async address =>
        {
            try { return await ProbeAsync(address, cancellationToken).ConfigureAwait(false); }
            catch { return null; }
        });

        return (await Task.WhenAll(probes).ConfigureAwait(false))
            .Where(device => device is not null)
            .Cast<SamsungTizenDevice>()
            .OrderBy(device => device.Name, StringComparer.CurrentCultureIgnoreCase)
            .ToArray();
    }

    public static string ParseHeader(string response, string headerName)
    {
        foreach (var line in response.Split(["\r\n", "\n"], StringSplitOptions.RemoveEmptyEntries))
        {
            var separator = line.IndexOf(':');
            if (separator <= 0 || !line[..separator].Equals(headerName, StringComparison.OrdinalIgnoreCase)) continue;
            return line[(separator + 1)..].Trim();
        }
        return "";
    }
}
