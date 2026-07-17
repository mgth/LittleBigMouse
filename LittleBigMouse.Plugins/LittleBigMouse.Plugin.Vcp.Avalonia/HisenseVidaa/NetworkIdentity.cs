#nullable enable
using System.Net;
using System.Net.NetworkInformation;
using System.Net.Sockets;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

static class NetworkIdentity
{
    public static string ControllerMacFor(string remoteAddress)
    {
        if (!IPAddress.TryParse(remoteAddress, out var remote) || remote.AddressFamily != AddressFamily.InterNetwork)
            throw new ArgumentException("Enter a valid projector IPv4 address.", nameof(remoteAddress));

        using var socket = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
        socket.Connect(new IPEndPoint(remote, HisenseVidaaProtocol.MqttPort));
        var local = ((IPEndPoint)socket.LocalEndPoint!).Address;
        foreach (var network in NetworkInterface.GetAllNetworkInterfaces())
        foreach (var address in network.GetIPProperties().UnicastAddresses)
            if (address.Address.Equals(local))
                return HisenseVidaaProtocol.NormalizeMac(network.GetPhysicalAddress().ToString());

        throw new InvalidOperationException($"Could not find the network interface used to reach {remoteAddress}.");
    }
}
