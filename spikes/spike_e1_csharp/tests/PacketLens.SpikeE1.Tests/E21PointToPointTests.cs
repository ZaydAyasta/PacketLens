using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class E21PointToPointTests
{
    [Fact]
    public void SwitchCannotBePeerEvenWithIpv4Metadata()
    {
        var e=new SimulationEngine(); e.CreateHost("h",["p"]); e.CreateL2Switch("sw",["p"]);
        foreach (var (name,last) in new[] { ("h",0),("sw",1) })
        {
            e.SetInterfaceIpv4Mode(Endpoint.Of(name,"p"),Ipv4LinkMode.PointToPoint);
            e.ConfigureInterface(Endpoint.Of(name,"p"),MacAddress.Parse($"02:00:00:00:00:{last+1:x2}"),Ip($"172.16.0.{last}"),31,true);
        }
        e.ConnectLink("wire",Endpoint.Of("h","p"),Endpoint.Of("sw","p"));
        Assert.Equal(NetworkConfigurationError.InvalidPointToPointTopology,Assert.Throws<NetworkConfigurationException>(e.ValidateIpv4Topology).Code);
        Assert.Empty(e.Trace.Records);
    }
    private static Ipv4Address Ip(string text) => Ipv4Address.Parse(text);
    private static SimulationEngine Pair(int last=1,int prefix=31,bool peerMode=true)
    {
        var e=new SimulationEngine();
        foreach (var (name,address,length,mode) in new[] { ("h0",0,31,true), ("h1",last,prefix,peerMode) })
        {
            e.CreateHost(name,["p"]);
            if (mode) e.SetInterfaceIpv4Mode(Endpoint.Of(name,"p"),Ipv4LinkMode.PointToPoint);
            e.ConfigureInterface(Endpoint.Of(name,"p"),MacAddress.Parse($"02:00:00:00:00:{address+1:x2}"),Ip($"172.16.0.{address}"),(byte)length,true);
        }
        e.ConnectLink("wire",Endpoint.Of("h0","p"),Endpoint.Of("h1","p")); return e;
    }

    [Fact]
    public void DirectSlash31PairAcceptsBothEndpointsAndExistingBidirectionalPing()
    {
        var e=Pair(); e.ValidateIpv4Topology();
        e.SendPing("h0",Ip("172.16.0.1"),1,1); e.RunUntilQuiescent(100);
        e.SendPing("h1",Ip("172.16.0.0"),2,1); e.RunUntilQuiescent(100);
        foreach (var host in new[] { "h0","h1" }) Assert.Single(e.Trace.Records.OfType<PingCompleted>(),p => p.Host==host);
        Assert.Empty(e.Trace.Records.OfType<PingFailed>());
    }

    [Theory]
    [InlineData(0,31,true)] [InlineData(2,31,true)] [InlineData(1,30,true)] [InlineData(1,31,false)]
    public void RejectsEqualAddressWrongPairPrefixAndMissingMode(int last,int prefix,bool mode)
    {
        var e=Pair(last,prefix,mode);
        Assert.Equal(NetworkConfigurationError.InvalidPointToPointTopology,Assert.Throws<NetworkConfigurationException>(e.ValidateIpv4Topology).Code);
        Assert.Equal(NetworkConfigurationError.InvalidPointToPointTopology,Assert.Throws<NetworkConfigurationException>(() => e.SendPing("h0",Ip("172.16.0.1"),1,1)).Code);
        Assert.Empty(e.Trace.Records); Assert.Equal(0UL,e.CurrentTimeNs);
    }

    [Fact]
    public void RejectsSwitchPathAndRevalidatesDisconnectReconnect()
    {
        var e=Pair(); e.DisconnectLink("wire"); Assert.Throws<NetworkConfigurationException>(e.ValidateIpv4Topology);
        e.CreateL2Switch("sw",["a","b"]); e.ConnectLink("left",Endpoint.Of("h0","p"),Endpoint.Of("sw","a")); e.ConnectLink("right",Endpoint.Of("h1","p"),Endpoint.Of("sw","b"));
        Assert.Throws<NetworkConfigurationException>(e.ValidateIpv4Topology);
        e.DisconnectLink("left"); e.DisconnectLink("right"); e.ConnectLink("wire",Endpoint.Of("h0","p"),Endpoint.Of("h1","p")); e.ValidateIpv4Topology();
        e.SetInterfaceAdminState(Endpoint.Of("h1","p"),false); e.ValidateIpv4Topology();
        Assert.True(e.InterfaceIsOperational(Endpoint.Of("h0","p"))); Assert.False(e.InterfaceIsOperational(Endpoint.Of("h1","p")));
    }

    [Fact]
    public void RouterSlash31AttachmentStaticRouteAndBusyMode()
    {
        var e=Pair();
        e.CreateRouter("r",[
            new("a",MacAddress.Parse("02:00:00:00:00:03"),new(Ip("10.0.0.0"),31),true,Ipv4LinkMode.PointToPoint),
            new("b",MacAddress.Parse("02:00:00:00:00:04"),new(Ip("192.168.1.1"),24),true)
        ]);
        Assert.Throws<NetworkConfigurationException>(e.ValidateIpv4Topology);
        e.CreateHost("peer",["p"]); e.SetInterfaceIpv4Mode(Endpoint.Of("peer","p"),Ipv4LinkMode.PointToPoint);
        e.ConfigureInterface(Endpoint.Of("peer","p"),MacAddress.Parse("02:00:00:00:00:05"),Ip("10.0.0.1"),31,true);
        e.ConnectLink("routerwire",Endpoint.Of("r","a"),Endpoint.Of("peer","p"));
        e.AddRouterStaticRoute("r",new(Ip("203.0.113.0"),31),null,"b",0); e.ValidateIpv4Topology();
        Assert.Equal("a",e.LookupRouterRoute("r",Ip("10.0.0.1"))!.EgressInterface);
        e.SendPing("h0",Ip("172.16.0.1"),1,1);
        Assert.Equal(SimulationErrorCode.EngineBusy,Assert.Throws<SimulationException>(() => e.SetInterfaceIpv4Mode(Endpoint.Of("r","a"),Ipv4LinkMode.MultiAccess)).Code);
        e.RunUntilQuiescent(100); e.ValidateIpv4Topology();
    }
    [Fact]
    public void TrafficAdmissionRejectsIncompleteSlash31WithoutAnyEffect()
    {
        var e=new SimulationEngine(); e.CreateHost("h",["p"]);
        e.ConfigureInterface(Endpoint.Of("h","p"),MacAddress.Parse("02:00:00:00:00:01"),Ipv4Address.Parse("172.16.0.0"),31,true);
        Assert.Throws<NetworkConfigurationException>(() => e.SendPing("h",Ipv4Address.Parse("172.16.0.1"),1,1));
        Assert.Empty(e.Trace.Records); Assert.Equal(0UL,e.CurrentTimeNs);
    }

    [Fact]
    public void GlobalGatePreservesUnrelatedPingAndDoesNotBlockPendingLinkFailure()
    {
        var e=Pair(); e.CreateHost("unused",["p"]);
        var mac=MacAddress.Parse("02:00:00:00:00:09");
        e.ConfigureInterface(Endpoint.Of("unused","p"),mac,Ip("10.0.0.0"),31,false);
        Assert.Equal(NetworkConfigurationError.InvalidPointToPointTopology,Assert.Throws<NetworkConfigurationException>(() => e.SendPing("h0",Ip("172.16.0.1"),7,1)).Code);
        Assert.Empty(e.Trace.Records);
        e.ConfigureInterface(Endpoint.Of("unused","p"),mac,Ip("10.0.0.1"),24,false);
        e.SendPing("h0",Ip("172.16.0.1"),7,1); e.StepEvents(1); e.DisconnectLink("wire");
        Assert.Equal(ExecutionOutcome.Quiescent,e.RunUntilQuiescent(100).Outcome);
        Assert.Contains(e.Trace.Records.OfType<PacketDropped>(),p => p.Reason==DropReason.LinkDisconnectedInFlight);
        Assert.Single(e.Trace.Records.OfType<PingFailed>(),p => p.Host=="h0");
    }

    [Fact]
    public void RouterToRouterPairIsStructuralNotAHostSpecialCase()
    {
        var e=new SimulationEngine();
        foreach (var (name,last,lan) in new[] { ("r0",0,10), ("r1",1,20) })
            e.CreateRouter(name,[
                new("transit",MacAddress.Parse($"02:00:00:00:01:{last:x2}"),new(Ip($"198.51.100.{last}"),31),true,Ipv4LinkMode.PointToPoint),
                new("lan",MacAddress.Parse($"02:00:00:00:02:{last:x2}"),new(Ip($"192.168.{lan}.1"),24),true)
            ]);
        e.ConnectLink("transit",Endpoint.Of("r0","transit"),Endpoint.Of("r1","transit")); e.ValidateIpv4Topology();
        Assert.True(e.InterfaceIsOperational(Endpoint.Of("r0","transit"))); Assert.True(e.InterfaceIsOperational(Endpoint.Of("r1","transit")));
        Assert.Equal(2,e.RouterRoutes("r0").Count); Assert.Empty(e.Trace.Records);
    }
}
