using LittleBigMouse.Plugin.Vcp.Networking;
using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

public class Ipv4AddressTests
{
    [Theory]
    [InlineData("192.168.1.10")]
    [InlineData("10.0.0.1")]
    [InlineData("255.255.255.255")]
    [InlineData(" 192.168.1.10 ")]          // pasted into a text box
    [InlineData("\t192.168.1.10\n")]
    public void AcceptsLiteralIpv4Addresses(string value) => Assert.True(Ipv4Address.IsValid(value));

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData("192.168.1.256")]
    [InlineData("192.168.1.10.4")]
    [InlineData("tv.local")]                // a name would move the failure to the first command
    [InlineData("::1")]                     // IPv6: these devices are only reached over IPv4
    [InlineData("2001:db8::1")]
    [InlineData("192.168.1.10:8002")]       // a port is not part of the address
    public void RejectsWhatCannotBeDialled(string? value) => Assert.False(Ipv4Address.IsValid(value));

    [Fact]
    public void TryParseReturnsTheAddressSoCallersCanReadItsOctets()
    {
        Assert.True(Ipv4Address.TryParse(" 192.168.7.42 ", out var address));
        Assert.Equal(new byte[] { 192, 168, 7, 42 }, address.GetAddressBytes());
    }

    [Fact]
    public void TryParseYieldsNoAddressWhenItFails()
    {
        Assert.False(Ipv4Address.TryParse("2001:db8::1", out var address));
        Assert.Null(address);
    }

    [Fact]
    public void RequireReturnsTheAddressWithoutItsBlanksSoTheStoredTextIsTheDialledText()
        => Assert.Equal("192.168.1.10", Ipv4Address.Require(" 192.168.1.10 ", "ipAddress"));

    [Fact]
    public void RequireNamesTheRejectedArgument()
    {
        var error = Assert.Throws<ArgumentException>(() => Ipv4Address.Require("tv.local", "ipAddress"));

        Assert.Equal("ipAddress", error.ParamName);
        Assert.Contains(Ipv4Address.InvalidMessage, error.Message);
    }

    [Fact]
    public void RequireKeepsTheCallerMessageWhenItHasOneOfItsOwn()
    {
        var error = Assert.Throws<ArgumentException>(
            () => Ipv4Address.Require("", "ipAddress", "A valid IPv4 address is required."));

        Assert.Contains("A valid IPv4 address is required.", error.Message);
        Assert.DoesNotContain(Ipv4Address.InvalidMessage, error.Message);
    }
}
