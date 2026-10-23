using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class E21RouterIngressTests
{
    private static Ipv4Address Ip(string s)=>Ipv4Address.Parse(s);
    private static MacAddress Mac(string s)=>MacAddress.Parse(s);
    private static SimulationEngine Fixture()
    {
        var e=new SimulationEngine();
        foreach (var (name,address,mac) in new[] {("h1","192.168.1.10","02:00:00:00:01:01"),("h2","10.0.0.20","02:00:00:00:02:02")})
        { e.CreateHost(name,["p"]); e.ConfigureHostIpv4(Endpoint.Of(name,"p"),Mac(mac),new(new(Ip(address),24)),true); }
        e.CreateRouter("r",[new("a",Mac("02:00:00:00:01:fe"),new(Ip("192.168.1.1"),24),true),new("b",Mac("02:00:00:00:02:fe"),new(Ip("10.0.0.1"),24),true)]);
        e.ConnectLink("left",Endpoint.Of("h1","p"),Endpoint.Of("r","a")); e.ConnectLink("right",Endpoint.Of("r","b"),Endpoint.Of("h2","p")); return e;
    }
    private static void Inject(SimulationEngine e,int ttl,string source,string target,bool broadcast)=>e.InjectFrameForTesting(Endpoint.Of("r","a"),new(Mac("02:00:00:00:01:01"),broadcast?MacAddress.Broadcast:Mac("02:00:00:00:01:fe"),new Ipv4Packet(Ip(source),Ip(target),(byte)ttl,new IcmpPacket(IcmpKind.EchoRequest,42,1))),0);

    [Theory]
    [InlineData(0,"192.168.1.1")][InlineData(1,"192.168.1.1")][InlineData(0,"10.0.0.1")][InlineData(1,"10.0.0.1")]
    public void LocalEchoBeforeTransitTtlUsesRequestedIp(int ttl,string target)
    {
        var e=Fixture(); Inject(e,ttl,"192.168.1.10",target,false); e.RunUntilQuiescent(100);
        Assert.Single(e.Trace.Records.OfType<FrameTransmitted>(),f=>f.From.Device=="r" && f.Frame.Payload is Ipv4Packet p && p.SourceIp==Ip(target) && p.Ttl==64);
        Assert.Empty(e.Trace.Records.OfType<PingCompleted>()); Assert.Empty(e.Trace.Records.OfType<PingFailed>());
    }

    [Theory]
    [InlineData(64,"192.168.1.10","192.168.1.1",true)][InlineData(64,"0.0.0.0","192.168.1.1",false)]
    [InlineData(64,"192.168.1.255","192.168.1.1",false)][InlineData(64,"192.168.1.10","10.0.0.255",false)][InlineData(64,"192.168.1.10","224.0.0.1",false)]
    public void UnsupportedErrorPathsFailClosedBeforeForwardingOrArp(int ttl,string source,string target,bool broadcast)
    {
        var e=Fixture(); Inject(e,ttl,source,target,broadcast);
        Assert.Equal(SimulationErrorCode.RouterIcmpErrorPathPending,Assert.Throws<SimulationException>(()=>e.StepEvents(1)).Code);
        Assert.IsType<FrameDelivered>(Assert.Single(e.Trace.Records)); Assert.Equal(0UL,e.CurrentTimeNs);
    }

    [Fact]
    public void RenewedRouterArpIgnoresOldExpiryAndExpiresAtNewEpoch()
    {
        var e=Fixture();var neighbor=Mac("02:00:00:00:01:01");
        void InjectArp(ulong at)=>e.InjectFrameForTesting(Endpoint.Of("r","a"),new(neighbor,MacAddress.Broadcast,new ArpPacket(ArpOperation.Request,neighbor,Ip("192.168.1.10"),MacAddress.Zero,Ip("192.168.1.1"))),at);
        InjectArp(0);e.RunUntilQuiescent(100);e.AdvanceUntil(10000,100);InjectArp(10000);e.RunUntilQuiescent(100);var before=e.Trace.Records.ToArray();
        e.AdvanceUntil(SimulationEngine.ArpCacheTtlNs,100);Assert.Equal(before,e.Trace.Records);Assert.Equal(neighbor,e.RouterArpEntry(Endpoint.Of("r","a"),Ip("192.168.1.10")));
        e.AdvanceUntil(SimulationEngine.ArpCacheTtlNs+10000,100);Assert.Null(e.RouterArpEntry(Endpoint.Of("r","a"),Ip("192.168.1.10")));
        Assert.Contains(e.Trace.Records.OfType<TableUpdated>(),r=>r.AtNs==SimulationEngine.ArpCacheTtlNs+10000 && r.Device=="r" && r.ObservationInterface=="a" && r.Update is ArpEntryExpired);
    }
}
