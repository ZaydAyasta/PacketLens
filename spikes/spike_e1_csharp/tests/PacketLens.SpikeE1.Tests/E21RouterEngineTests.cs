using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class E21RouterEngineTests
{
    private static Ipv4Address Ip(string value) => Ipv4Address.Parse(value);
    private static RouterPortConfiguration[] Ports() => [
        new("a",MacAddress.Parse("02:00:00:00:00:01"),new(Ip("192.168.1.1"),24),true),
        new("b",MacAddress.Parse("02:00:00:00:00:02"),new(Ip("172.16.1.1"),24),true)
    ];
    private static SimulationEngine Engine() { var e=new SimulationEngine(); e.CreateRouter("r",Ports()); return e; }

    [Fact]
    public void RegistrationIsAtomicAndReservesIdentifiers()
    {
        var e=Engine();
        Assert.Equal(2,e.RouterRoutes("r").Count);
        Assert.Equal(Ip("192.168.1.1"),e.InterfaceIpv4(Endpoint.Of("r","a"))!.Address);
        Assert.Throws<ArgumentException>(() => e.CreateRouter("r",Ports()));
        Assert.Throws<NetworkConfigurationException>(() => e.CreateRouter("bad",[]));
        e.CreateHost("bad",["p"]);
        var duplicate=Ports(); duplicate[1]=duplicate[1] with { Name="a" };
        Assert.Throws<ArgumentException>(() => e.CreateRouter("duplicate",duplicate));
        e.CreateHost("duplicate",["p"]);
        Assert.Equal(0UL,e.CurrentTimeNs);
        Assert.Empty(e.Trace.Records);
    }

    [Fact]
    public void LpmTracksLiveAdminAndLinkGenerationsWithoutDeletingRoutes()
    {
        var e=Engine();
        e.CreateHost("peer",["x","y"]);
        e.AddRouterStaticRoute("r",new(Ip("10.0.0.0"),8),Ip("172.16.1.2"),"b",0);
        e.AddRouterStaticRoute("r",new(Ip("0.0.0.0"),0),Ip("192.168.1.2"),"a",99);
        string Selected() => e.LookupRouterRoute("r",Ip("10.1.1.1"))!.EgressInterface;
        Assert.Null(e.LookupRouterRoute("r",Ip("10.1.1.1")));
        e.ConnectLink("left",Endpoint.Of("r","a"),Endpoint.Of("peer","x"));
        Assert.Equal("a",Selected());
        e.ConnectLink("right",Endpoint.Of("r","b"),Endpoint.Of("peer","y"));
        Assert.Equal("b",Selected());
        e.SetInterfaceAdminState(Endpoint.Of("r","b"),false);
        Assert.Equal("a",Selected());
        e.SetInterfaceAdminState(Endpoint.Of("r","b"),true);
        Assert.Equal("b",Selected());
        e.SetInterfaceAdminState(Endpoint.Of("peer","y"),false);
        Assert.Equal("b",Selected());
        e.DisconnectLink("right"); Assert.Equal("a",Selected());
        e.ConnectLink("right",Endpoint.Of("r","b"),Endpoint.Of("peer","y")); Assert.Equal("b",Selected());
        e.DisconnectLink("right"); e.DisconnectLink("left");
        Assert.Null(e.LookupRouterRoute("r",Ip("10.1.1.1")));
        Assert.Equal(4,e.RouterRoutes("r").Count);
        Assert.Empty(e.Trace.Records);
        Assert.Equal(0UL,e.CurrentTimeNs);
    }

    [Fact]
    public void LegacyReconfigurationSynchronizesConnectedRoutesAndRollsBackErrors()
    {
        var e=Engine(); var endpoint=Endpoint.Of("r","b"); var mac=MacAddress.Parse("02:00:00:00:00:02");
        e.CreateHost("peer",["p"]); e.ConnectLink("wire",endpoint,Endpoint.Of("peer","p"));
        var before=e.RouterRoutes("r").ToArray();
        Assert.Equal(NetworkConfigurationError.DuplicateConnectedPrefix,Assert.Throws<NetworkConfigurationException>(() => e.ConfigureInterface(endpoint,mac,Ip("192.168.1.2"),24,false)).Code);
        Assert.Equal(before,e.RouterRoutes("r")); Assert.Equal(Ip("172.16.1.1"),e.InterfaceIpv4(endpoint)!.Address);
        Assert.True(e.InterfaceIsOperational(endpoint));
        Assert.Throws<NetworkConfigurationException>(() => e.ConfigureInterface(endpoint,mac,null,24,false));
        e.ConfigureInterface(endpoint,mac,Ip("172.17.1.1"),24,true);
        Assert.Contains(e.RouterRoutes("r"),r => r.Network==new Ipv4Network(Ip("172.17.1.0"),24));
        e.AddRouterStaticRoute("r",new(Ip("10.0.0.0"),8),Ip("172.17.1.2"),"b",7);
        before=e.RouterRoutes("r").ToArray();
        Assert.Equal(NetworkConfigurationError.NextHopNotDirectlyConnected,Assert.Throws<NetworkConfigurationException>(() => e.ConfigureInterface(endpoint,mac,Ip("172.18.1.1"),24,false)).Code);
        Assert.Equal(before,e.RouterRoutes("r")); Assert.Equal(Ip("172.17.1.1"),e.InterfaceIpv4(endpoint)!.Address);
        Assert.True(e.InterfaceIsOperational(endpoint));
        Assert.Empty(e.Trace.Records); Assert.Equal(0UL,e.CurrentTimeNs);
    }

    private static void ActiveHosts(SimulationEngine e)
    {
        foreach (var (host,last) in new[] { ("h",10), ("other",20) })
        {
            e.CreateHost(host,["eth0"]);
            e.ConfigureInterface(Endpoint.Of(host,"eth0"),MacAddress.Parse($"02:00:00:00:00:{last:x2}"),Ip($"192.168.5.{last}"),24,true);
        }
        e.ConnectLink("traffic",Endpoint.Of("h","eth0"),Endpoint.Of("other","eth0"));
        e.SendPing("h",Ip("192.168.5.20"),1,1);
    }

    [Fact]
    public void RouterAndRouteMutationsAreBusyAtomicButAdminAndLinksRemainAllowed()
    {
        var e=Engine(); ActiveHosts(e);
        var before=e.Trace.Records.ToArray(); var routes=e.RouterRoutes("r").ToArray();
        Assert.Equal(SimulationErrorCode.EngineBusy,Assert.Throws<SimulationException>(() => e.AddRouterStaticRoute("r",new(Ip("10.0.0.0"),8),null,"a",1)).Code);
        Assert.Equal(SimulationErrorCode.EngineBusy,Assert.Throws<SimulationException>(() => e.ConfigureInterface(Endpoint.Of("r","a"),MacAddress.Parse("02:00:00:00:00:99"),Ip("192.168.2.1"),24,false)).Code);
        Assert.Equal(SimulationErrorCode.EngineBusy,Assert.Throws<SimulationException>(() => e.CreateRouter("new",Ports())).Code);
        Assert.Equal(routes,e.RouterRoutes("r")); Assert.Equal(Ip("192.168.1.1"),e.InterfaceIpv4(Endpoint.Of("r","a"))!.Address);
        Assert.Equal(before,e.Trace.Records); Assert.Equal(0UL,e.CurrentTimeNs);
        e.SetInterfaceAdminState(Endpoint.Of("r","a"),false);
        e.CreateHost("spare",["p"]); e.ConnectLink("sparelink",Endpoint.Of("r","a"),Endpoint.Of("spare","p")); e.DisconnectLink("sparelink");
        Assert.Equal(ExecutionOutcome.Quiescent,e.RunUntilQuiescent(100).Outcome);
        var after=e.Trace.Records.ToArray(); var time=e.CurrentTimeNs;
        e.AddRouterStaticRoute("r",new(Ip("10.0.0.0"),8),null,"a",1);
        e.ConfigureInterface(Endpoint.Of("r","b"),MacAddress.Parse("02:00:00:00:00:02"),Ip("172.17.1.1"),24,true);
        e.CreateRouter("new",Ports());
        Assert.Equal(after,e.Trace.Records); Assert.Equal(time,e.CurrentTimeNs);
    }

    [Fact]
    public void RouterUsesScopedArpWithoutL2SwitchLearning()
    {
        var e=Engine(); e.CreateHost("h",["p"]);
        e.ConfigureInterface(Endpoint.Of("h","p"),MacAddress.Parse("02:00:00:00:00:10"),Ip("192.168.1.10"),24,true);
        e.ConnectLink("wire",Endpoint.Of("h","p"),Endpoint.Of("r","a"));
        e.SendPing("h",Ip("192.168.1.1"),1,1);
        Assert.Equal(ExecutionOutcome.Quiescent,e.RunUntilQuiescent(100).Outcome);
        Assert.Equal(0,e.MacTableEntryCount("r")); Assert.Null(e.ArpEntry("r",Ip("192.168.1.10")));
        Assert.Equal(MacAddress.Parse("02:00:00:00:00:10"),e.RouterArpEntry(Endpoint.Of("r","a"),Ip("192.168.1.10")));
    }

    [Fact]
    public void ExistingLinkGuardsAndInvalidInputsPreserveRouterConfiguration()
    {
        var e=Engine(); var before=e.RouterRoutes("r").ToArray();
        Assert.Throws<ArgumentException>(() => e.ConnectLink("self",Endpoint.Of("r","a"),Endpoint.Of("r","a")));
        Assert.Throws<ArgumentException>(() => e.ConnectLink("unknown",Endpoint.Of("r","a"),Endpoint.Of("r","missing")));
        Assert.False(e.InterfaceIsOperational(Endpoint.Of("r","a")));
        Assert.Throws<NetworkConfigurationException>(() => e.AddRouterStaticRoute("r",new(Ip("10.0.0.0"),8),null,"absent",0));
        Assert.Throws<ArgumentException>(() => e.ConfigureInterface(Endpoint.Of("r","absent"),MacAddress.Parse("02:00:00:00:00:01"),Ip("10.0.0.1"),24,true));
        Assert.Equal(before,e.RouterRoutes("r"));
        var duplicate=Ports(); duplicate[1]=duplicate[1] with { Ipv4=new(Ip("192.168.1.2"),24), AdminUp=false };
        Assert.Equal(NetworkConfigurationError.DuplicateConnectedPrefix,Assert.Throws<NetworkConfigurationException>(() => e.CreateRouter("invalid",duplicate)).Code);
        e.CreateHost("invalid",["p"]);
        Assert.Throws<ArgumentException>(() => e.CreateRouter("invalid",Ports()));
        Assert.Throws<ArgumentException>(() => e.RouterRoutes("invalid")); Assert.Throws<ArgumentException>(() => e.RouterRoutes("absent"));
        Assert.Throws<ArgumentException>(() => e.LookupRouterRoute("invalid",Ip("10.0.0.1")));
        e.ConnectLink("good",Endpoint.Of("r","a"),Endpoint.Of("invalid","p")); Assert.True(e.InterfaceIsOperational(Endpoint.Of("r","a")));
        Assert.Throws<InvalidOperationException>(() => e.ConnectLink("another",Endpoint.Of("r","a"),Endpoint.Of("r","b")));
        Assert.Throws<ArgumentException>(() => e.ConnectLink("good",Endpoint.Of("r","b"),Endpoint.Of("invalid","p")));
        var detached=e.RouterRoutes("r").ToList(); detached.Clear(); Assert.Equal(before,e.RouterRoutes("r")); Assert.Empty(e.Trace.Records);
    }
}
