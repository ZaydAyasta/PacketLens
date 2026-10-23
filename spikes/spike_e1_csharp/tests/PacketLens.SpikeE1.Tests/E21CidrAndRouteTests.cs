using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class E21CidrAndRouteTests
{
    private static Ipv4Address Ip(string value) => Ipv4Address.Parse(value);
    private static RouterConfiguration Router() => new(new Dictionary<string,Ipv4InterfaceAddress>
    {
        ["eth0"] = new(Ip("192.168.1.1"),24),
        ["eth1"] = new(Ip("172.16.1.1"),24)
    });

    [Fact]
    public void Rt02NormalizedDuplicatesRejectWithoutMutation()
    {
        var router = Router();
        router.AddStaticRoute(new Ipv4Network(Ip("10.1.1.99"),24),Ip("172.16.1.2"),"eth1",1);
        var before = router.Routes.ToArray();
        Assert.Throws<NetworkConfigurationException>(() => router.AddStaticRoute(new Ipv4Network(Ip("10.1.1.0"),24),Ip("172.16.1.3"),"eth1",999));
        Assert.Equal(before,router.Routes);
        Assert.Throws<NetworkConfigurationException>(() => router.AddStaticRoute(new Ipv4Network(Ip("192.168.1.77"),24),null,"eth0",0));
        Assert.Equal(before,router.Routes);
    }

    [Fact]
    public void Rt03DuplicateConnectedAndInvalidRouterReject()
    {
        Assert.Throws<NetworkConfigurationException>(() => new RouterConfiguration(new Dictionary<string,Ipv4InterfaceAddress>()));
        Assert.Throws<NetworkConfigurationException>(() => new RouterConfiguration(new Dictionary<string,Ipv4InterfaceAddress> { ["eth0"] = new(Ip("10.0.0.1"),24) }));
        Assert.Throws<NetworkConfigurationException>(() => new RouterConfiguration(new Dictionary<string,Ipv4InterfaceAddress>
        {
            ["eth0"] = new(Ip("10.0.0.1"),24), ["eth1"] = new(Ip("10.0.0.2"),24)
        }));
    }

    [Fact]
    public void Rt04NextHopValidationPreservesTable()
    {
        var router = Router();
        var before = router.Routes.ToArray();
        foreach (var (name,nextHop) in new[] { ("missing","172.16.1.2"), ("eth1","192.168.1.2"), ("eth1","172.16.1.1"), ("eth1","172.16.1.0"), ("eth1","172.16.1.255"), ("eth1","224.0.0.1") })
        {
            Assert.Throws<NetworkConfigurationException>(() => router.AddStaticRoute(new Ipv4Network(Ip("10.0.0.0"),8),Ip(nextHop),name,0));
            Assert.Equal(before,router.Routes);
        }
        router.AddStaticRoute(new Ipv4Network(Ip("10.0.0.0"),8),null,"eth1",0);
        Assert.Null(router.LookupRoute(Ip("10.1.1.1"),_ => true)!.NextHop);
    }

    [Fact]
    public void Rt03ReconfigurationChecksWholeTableBeforeCommit()
    {
        var router = Router();
        router.AddStaticRoute(new Ipv4Network(Ip("10.20.0.0"),16),Ip("172.16.1.2"),"eth1",7);
        var before = router.Routes.ToArray();
        foreach (var (name,address,prefix) in new[] { ("eth1","192.168.1.2",24), ("eth0","10.20.0.1",16), ("eth1","172.17.1.1",24), ("missing","172.17.1.1",24) })
        {
            Assert.Throws<NetworkConfigurationException>(() => router.ConfigureInterface(name,new Ipv4InterfaceAddress(Ip(address),prefix)));
            Assert.Equal(before,router.Routes);
        }
        router.ConfigureInterface("eth0",new Ipv4InterfaceAddress(Ip("192.168.2.1"),24));
        Assert.Null(router.LookupRoute(Ip("192.168.1.10"),_ => true));
        Assert.True(router.LookupRoute(Ip("192.168.2.10"),_ => true)!.IsConnected);
        Assert.Equal(Ip("172.16.1.2"),router.LookupRoute(Ip("10.20.1.5"),_ => true)!.NextHop);
    }

    [Fact]
    public void Rt05SpecificStaticOverridesConnectedAndDefault()
    {
        var router = Router();
        router.AddStaticRoute(new Ipv4Network(Ip("0.0.0.0"),0),Ip("172.16.1.2"),"eth1",0);
        router.AddStaticRoute(new Ipv4Network(Ip("192.168.1.64"),28),Ip("172.16.1.2"),"eth1",999);
        router.AddStaticRoute(new Ipv4Network(Ip("192.168.1.70"),32),Ip("172.16.1.3"),"eth1",1000);
        foreach (var (target,prefix) in new[] { ("192.168.1.70",32), ("192.168.1.71",28), ("192.168.1.10",24), ("198.51.100.1",0) })
            Assert.Equal(prefix,router.LookupRoute(Ip(target),_ => true)!.Network.Prefix.Value);
    }

    [Theory]
    [InlineData("224.0.0.1")]
    [InlineData("127.0.0.1")]
    [InlineData("0.0.0.0")]
    [InlineData("192.168.1.1")]
    public void Rt04InvalidNextHopReason(string nextHop)
    {
        var router = Router();
        var before = router.Routes.ToArray();
        var error = Assert.Throws<NetworkConfigurationException>(() => router.AddStaticRoute(new Ipv4Network(Ip("10.0.0.0"),8),Ip(nextHop),"eth1",0));
        Assert.Equal(NetworkConfigurationError.InvalidNextHop,error.Code);
        Assert.Equal(before,router.Routes);
    }

    [Fact]
    public void RouteValidationErrorCodesAreContractual()
    {
        var router = Router();
        var network = new Ipv4Network(Ip("10.0.0.0"),8);
        Assert.Equal(NetworkConfigurationError.UnknownInterface,Assert.Throws<NetworkConfigurationException>(() => router.AddStaticRoute(network,null,"absent",0)).Code);
        Assert.Equal(NetworkConfigurationError.NextHopNotDirectlyConnected,Assert.Throws<NetworkConfigurationException>(() => router.AddStaticRoute(network,Ip("172.17.1.2"),"eth1",0)).Code);
        router.AddStaticRoute(network,null,"eth1",0);
        Assert.Equal(NetworkConfigurationError.AmbiguousRouteConfig,Assert.Throws<NetworkConfigurationException>(() => router.AddStaticRoute(network,null,"eth0",99)).Code);
        Assert.Equal(NetworkConfigurationError.PrefixOverlapsConnectedNetwork,Assert.Throws<NetworkConfigurationException>(() => router.AddStaticRoute(new Ipv4Network(Ip("192.168.1.90"),24),null,"eth1",99)).Code);
        Assert.Equal(NetworkConfigurationError.DuplicateConnectedPrefix,Assert.Throws<NetworkConfigurationException>(() => router.ConfigureInterface("eth1",new Ipv4InterfaceAddress(Ip("192.168.1.20"),24))).Code);
    }

    [Fact]
    public void Rt01ConnectedRoutesRemainConfiguredWhileInactive()
    {
        var router = Router();
        Assert.Equal(2,router.Routes.Count);
        Assert.All(router.Routes, r => { Assert.True(r.IsConnected); Assert.Equal(0U,r.Metric); Assert.Null(r.NextHop); });
        Assert.Equal(Ip("192.168.1.0"),router.LookupRoute(Ip("192.168.1.44"),_ => true)!.Network.Address);
        Assert.Null(router.LookupRoute(Ip("192.168.1.44"),_ => false));
        Assert.Equal(2,router.Routes.Count);
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Rt05LpmIgnoresMetricAndInsertion(bool reverse)
    {
        var router = Router();
        var entries = new[] { ("10.0.0.0",8,1U), ("10.1.0.0",16,2U), ("10.1.1.99",24,999U) };
        foreach (var (network,prefix,metric) in reverse ? entries.Reverse() : entries)
            router.AddStaticRoute(new Ipv4Network(Ip(network),prefix),Ip("172.16.1.2"),"eth1",metric);
        Assert.Equal(24,router.LookupRoute(Ip("10.1.1.50"),_ => true)!.Network.Prefix.Value);
        Assert.Equal(16,router.LookupRoute(Ip("10.1.2.50"),_ => true)!.Network.Prefix.Value);
        Assert.Equal(8,router.LookupRoute(Ip("10.2.2.50"),_ => true)!.Network.Prefix.Value);
        Assert.Null(router.LookupRoute(Ip("11.0.0.1"),_ => true));
    }

    [Fact]
    public void Rt06FallbackAndRecovery()
    {
        var router = Router();
        router.AddStaticRoute(new Ipv4Network(Ip("10.20.0.0"),16),Ip("172.16.1.2"),"eth1",5);
        router.AddStaticRoute(new Ipv4Network(Ip("0.0.0.0"),0),Ip("192.168.1.2"),"eth0",20);
        foreach (var (admin,link) in new[] { (false,true), (true,false), (false,false) })
            Assert.Equal("eth0",router.LookupRoute(Ip("10.20.1.5"),name => name == "eth0" || (admin && link))!.EgressInterface);
        Assert.Equal("eth1",router.LookupRoute(Ip("10.20.1.5"),_ => true)!.EgressInterface);
        Assert.Null(router.LookupRoute(Ip("10.20.1.5"),_ => false));
    }

    [Theory]
    [InlineData(0, 0U)]
    [InlineData(24, 0xFFFFFF00U)]
    [InlineData(31, 0xFFFFFFFEU)]
    [InlineData(32, uint.MaxValue)]
    public void Cidr01BoundsAndMasks(int length, uint expected)
    {
        var prefix = new Ipv4PrefixLength(length);
        Assert.Equal(length, prefix.Value);
        Assert.Equal(expected, prefix.Netmask);
    }

    [Theory]
    [InlineData(-1)]
    [InlineData(33)]
    [InlineData(255)]
    public void Cidr01RejectsInvalidPrefix(int length) =>
        Assert.Throws<ArgumentOutOfRangeException>(() => new Ipv4PrefixLength(length));

    [Fact]
    public void Cidr01AllMasksHaveExactlyDeclaredLeadingBits()
    {
        foreach (var length in Enumerable.Range(0,33))
        {
            var mask=new Ipv4PrefixLength(length).Netmask;
            Assert.Equal(length,System.Numerics.BitOperations.PopCount(mask));
            Assert.Equal(length,System.Numerics.BitOperations.LeadingZeroCount(~mask));
        }
    }

    [Theory]
    [InlineData("192.168.1.10",24,"192.168.1.0")]
    [InlineData("10.1.2.99",0,"0.0.0.0")]
    [InlineData("10.1.2.99",32,"10.1.2.99")]
    [InlineData("172.16.0.1",31,"172.16.0.0")]
    public void Cidr02NormalizationAndMembership(string value, int prefix, string expected)
    {
        var network = new Ipv4Network(Ipv4Address.Parse(value), prefix);
        Assert.Equal(Ipv4Address.Parse(expected), network.Address);
        Assert.True(network.Contains(Ipv4Address.Parse(value)));
        Assert.Equal(network, new Ipv4Network(network.Address, prefix));
        Assert.False(new Ipv4Network(Ipv4Address.Parse(value),32).Contains(Ipv4Address.Parse("203.0.113.99")));
    }

    [Fact]
    public void Cidr03Slash31AndSlash32DoNotHaveDirectedBroadcast()
    {
        var pair = new Ipv4InterfaceAddress(Ipv4Address.Parse("172.16.0.0"),31);
        Assert.True(pair.IsUsableOnLink(Ipv4Address.Parse("172.16.0.0")));
        Assert.True(pair.IsUsableOnLink(Ipv4Address.Parse("172.16.0.1")));
        Assert.Null(pair.Network.Broadcast);
        Assert.Null(new Ipv4Network(Ipv4Address.Parse("10.0.0.7"),32).Broadcast);
    }

    [Fact]
    public void Cfg01GatewayValidation()
    {
        var iface = new Ipv4InterfaceAddress(Ipv4Address.Parse("192.168.1.10"),24);
        var valid = new HostIpv4Settings(iface,Ipv4Address.Parse("192.168.1.1"));
        Assert.Equal(Ipv4Address.Parse("192.168.1.1"), valid.DefaultGateway);
        Assert.Null(new HostIpv4Settings(iface).DefaultGateway);
        foreach (var invalid in new[] { "192.168.2.1", "192.168.1.10", "192.168.1.0", "192.168.1.255", "0.0.0.0", "127.0.0.1", "224.0.0.1", "255.255.255.255" })
            Assert.Throws<NetworkConfigurationException>(() => new HostIpv4Settings(iface,Ipv4Address.Parse(invalid)));
        Assert.Throws<NetworkConfigurationException>(() => new Ipv4InterfaceAddress(Ipv4Address.Parse("192.168.1.255"),24));
    }
}
