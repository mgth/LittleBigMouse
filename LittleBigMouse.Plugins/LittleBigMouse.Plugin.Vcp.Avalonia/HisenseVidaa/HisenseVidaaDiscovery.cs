#nullable enable
using System.Net;
using System.Net.Http;
using System.Net.Sockets;
using System.Text;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

public sealed class HisenseVidaaDiscovery(HttpClient httpClient)
{
    static readonly IPEndPoint SsdpEndpoint = new(
        IPAddress.Parse(HisenseVidaaProtocol.SsdpAddress), HisenseVidaaProtocol.SsdpPort);

    public async Task<HisenseVidaaDevice> ProbeAsync(
        string ipAddress,
        CancellationToken cancellationToken = default)
    {
        if (!HisenseVidaaDevice.IsValidAddress(ipAddress))
            throw new ArgumentException("Enter a valid IPv4 address.", nameof(ipAddress));

        Exception? lastError = null;
        foreach (var port in HisenseVidaaProtocol.DescriptorPorts)
        {
            try
            {
                using var attempt = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
                attempt.CancelAfter(TimeSpan.FromSeconds(2));
                var xml = await httpClient.GetStringAsync(
                    $"http://{ipAddress}:{port}/MediaServer/rendererdevicedesc.xml", attempt.Token)
                    .ConfigureAwait(false);
                return HisenseVidaaProtocol.ParseDescriptor(ipAddress, xml);
            }
            catch (OperationCanceledException e) when (!cancellationToken.IsCancellationRequested)
            {
                lastError = e;
            }
            catch (HttpRequestException e)
            {
                lastError = e;
            }
        }

        throw new HttpRequestException(
            $"No VIDAA descriptor answered at {ipAddress} on ports {string.Join('/', HisenseVidaaProtocol.DescriptorPorts)}.",
            lastError);
    }

    public async Task<HisenseVidaaDevice> FindAsync(
        string lastKnownAddress,
        CancellationToken cancellationToken = default)
    {
        if (!IPAddress.TryParse(lastKnownAddress, out var seed)
            || seed.AddressFamily != AddressFamily.InterNetwork)
            throw new ArgumentException("Enter a valid projector IPv4 address.", nameof(lastKnownAddress));

        try
        {
            return await ProbeAsync(lastKnownAddress, cancellationToken).ConfigureAwait(false);
        }
        catch (HttpRequestException)
        {
            // SSDP multicast does not cross routers. Search only the /24 of the
            // last known address, and only the two VIDAA descriptor ports.
        }

        var octets = seed.GetAddressBytes();
        using var deadline = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        deadline.CancelAfter(TimeSpan.FromSeconds(8));
        using var concurrency = new SemaphoreSlim(48, 48);
        var probes = Enumerable.Range(1, 254)
            .Select(host => ProbeSubnetCandidateAsync(
                $"{octets[0]}.{octets[1]}.{octets[2]}.{host}", concurrency, deadline.Token))
            .ToArray();

        try
        {
            var devices = await Task.WhenAll(probes).ConfigureAwait(false);
            if (devices.FirstOrDefault(device => device is not null) is { } found) return found;
        }
        catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
        {
        }

        throw new HttpRequestException(
            $"No Hisense VIDAA device answered on {octets[0]}.{octets[1]}.{octets[2]}.0/24.");
    }

    async Task<HisenseVidaaDevice?> ProbeSubnetCandidateAsync(
        string address,
        SemaphoreSlim concurrency,
        CancellationToken cancellationToken)
    {
        await concurrency.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            foreach (var port in HisenseVidaaProtocol.DescriptorPorts)
            {
                using var tcp = new TcpClient(AddressFamily.InterNetwork);
                using var attempt = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
                attempt.CancelAfter(TimeSpan.FromMilliseconds(450));
                try
                {
                    await tcp.ConnectAsync(address, port, attempt.Token).ConfigureAwait(false);
                    return await ProbeAsync(address, cancellationToken).ConfigureAwait(false);
                }
                catch (Exception e) when (e is SocketException or HttpRequestException
                                                   or OperationCanceledException)
                {
                }
            }
            return null;
        }
        finally
        {
            concurrency.Release();
        }
    }

    public async Task<IReadOnlyList<HisenseVidaaDevice>> DiscoverAsync(
        TimeSpan timeout,
        CancellationToken cancellationToken = default)
    {
        using var udp = new UdpClient(AddressFamily.InterNetwork);
        foreach (var target in new[] { "urn:schemas-upnp-org:device:MediaRenderer:1", "ssdp:all" })
        {
            var request = Encoding.ASCII.GetBytes(
                "M-SEARCH * HTTP/1.1\r\n" +
                $"HOST: {HisenseVidaaProtocol.SsdpAddress}:{HisenseVidaaProtocol.SsdpPort}\r\n" +
                "MAN: \"ssdp:discover\"\r\n" +
                "MX: 3\r\n" +
                $"ST: {target}\r\n\r\n");
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
                addresses.Add(response.RemoteEndPoint.Address.ToString());
            }
        }
        catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
        {
        }

        var probes = addresses.Select(async address =>
        {
            try { return await ProbeAsync(address, cancellationToken).ConfigureAwait(false); }
            catch { return null; }
        });
        return (await Task.WhenAll(probes).ConfigureAwait(false))
            .Where(device => device is not null)
            .Cast<HisenseVidaaDevice>()
            .OrderBy(device => device.Name, StringComparer.CurrentCultureIgnoreCase)
            .ToArray();
    }
}
