using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class E21ForwardingTests
{
    private static Ipv4Address Ip(string text) => Ipv4Address.Parse(text);
    private static void Host(SimulationEngine e,string name,string ip,string? gateway,string mac)
    {
        e.CreateHost(name,["eth0"]);
        e.ConfigureHostIpv4(Endpoint.Of(name,"eth0"),MacAddress.Parse(mac),new(new(Ip(ip),24),gateway is null ? null : Ip(gateway)),true);
    }
    private static void Router(SimulationEngine e) => e.CreateRouter("r1",[
        new("eth0",MacAddress.Parse("00:00:00:00:01:fe"),new(Ip("192.168.1.1"),24),true),
        new("eth1",MacAddress.Parse("00:00:00:00:02:fe"),new(Ip("10.0.0.1"),24),true)
    ]);
    private static SimulationEngine S1Fixture()
    {
        var e=new SimulationEngine(); Host(e,"host1","192.168.1.10","192.168.1.1","00:00:00:00:01:01");
        e.CreateL2Switch("sw1",["port1","port2"]); Router(e); e.CreateL2Switch("sw2",["port1","port2"]);
        Host(e,"host2","10.0.0.20","10.0.0.1","00:00:00:00:02:02");
        foreach (var (sw,d) in new[] { ("sw1",2), ("sw2",4) }) for (var p=1;p<=2;p++) e.ConfigureInterface(Endpoint.Of(sw,$"port{p}"),MacAddress.Parse($"02:00:00:00:{d:x2}:{p:x2}"),null,24,true);
        foreach (var (link,a,pa,b,pb) in new[] { ("l1","host1","eth0","sw1","port1"),("l2","sw1","port2","r1","eth0"),("l3","r1","eth1","sw2","port1"),("l4","sw2","port2","host2","eth0") }) e.ConnectLink(link,Endpoint.Of(a,pa),Endpoint.Of(b,pb));
        return e;
    }

    [Fact]
    public void RemoteHostPingArpsOnlyGatewayAndQueuesOriginalIpDatagram()
    {
        var e=new SimulationEngine(); Host(e,"h","192.168.1.10","192.168.1.1","00:00:00:00:01:01");
        e.CreateL2Switch("sw",["p"]); e.ConnectLink("wire",Endpoint.Of("h","eth0"),Endpoint.Of("sw","p"));
        e.SendPing("h",Ip("10.0.0.20"),1,1); e.StepEvents(1);
        Assert.Contains(e.Trace.Records.OfType<ArpPacketQueued>(),q => q.TargetIp==Ip("192.168.1.1") && q.Packet.SourceIp==Ip("192.168.1.10") && q.Packet.DestinationIp==Ip("10.0.0.20") && q.Packet.Ttl==64);
        var request=Assert.Single(e.Trace.Records.OfType<FrameTransmitted>());
        var arp=Assert.IsType<ArpPacket>(request.Frame.Payload); Assert.Equal(ArpOperation.Request,arp.Operation); Assert.Equal(Ip("192.168.1.1"),arp.TargetIp);
        Assert.Equal(0UL,e.CurrentTimeNs);
    }

    [Fact]
    public void RouterAnswersOnlyIngressArpAndLearnsScopedEntry()
    {
        var e=new SimulationEngine(); Host(e,"h","192.168.1.10",null,"00:00:00:00:01:01"); Router(e);
        e.ConnectLink("wire",Endpoint.Of("h","eth0"),Endpoint.Of("r1","eth0"));
        e.SendPing("h",Ip("192.168.1.1"),1,1); e.AdvanceUntil(2200,100);
        Assert.Equal(MacAddress.Parse("00:00:00:00:01:fe"),e.ArpEntry("h",Ip("192.168.1.1")));
        Assert.Equal(MacAddress.Parse("00:00:00:00:01:01"),e.RouterArpEntry(Endpoint.Of("r1","eth0"),Ip("192.168.1.10")));
        Assert.Null(e.RouterArpEntry(Endpoint.Of("r1","eth1"),Ip("192.168.1.10")));
        Assert.Contains(e.Trace.Records.OfType<TableUpdated>(),r => r.Device=="r1" && r.ObservationInterface=="eth0" && r.Update is ArpLearned a && a.Ip==Ip("192.168.1.10"));
        Assert.DoesNotContain(e.Trace.Records.OfType<FrameTransmitted>(),r => r.From==Endpoint.Of("r1","eth1"));
    }

    [Fact]
    public void S1ColdRoundTripPreservesIpDecrementsTtlOnceAndMatchesTables()
    {
        var e=S1Fixture(); e.SendPing("host1",Ip("10.0.0.20"),1,1);
        Assert.Equal(ExecutionOutcome.Quiescent,e.RunUntilQuiescent(1000).Outcome);
        var completed=Assert.Single(e.Trace.Records.OfType<PingCompleted>()); Assert.Equal("host1",completed.Host); Assert.Equal(17500UL,completed.RttNs); Assert.Equal(17500UL,e.CurrentTimeNs);
        Assert.Empty(e.Trace.Records.OfType<PingFailed>()); Assert.Empty(e.Trace.Records.OfType<PacketDropped>());
        Assert.Equal(4,e.Trace.Records.OfType<TableUpdated>().Count(t=>t.Update is ArpLearned));
        foreach (var (device,port,source,destination,ttl,smac,dmac) in new[] {
            ("host1","eth0","192.168.1.10","10.0.0.20",64,"00:00:00:00:01:01","00:00:00:00:01:fe"),
            ("r1","eth1","192.168.1.10","10.0.0.20",63,"00:00:00:00:02:fe","00:00:00:00:02:02"),
            ("host2","eth0","10.0.0.20","192.168.1.10",64,"00:00:00:00:02:02","00:00:00:00:02:fe"),
            ("r1","eth0","10.0.0.20","192.168.1.10",63,"00:00:00:00:01:fe","00:00:00:00:01:01") })
        {
            var tx=Assert.Single(e.Trace.Records.OfType<FrameTransmitted>(),f => f.From==Endpoint.Of(device,port) && f.Frame.Payload is Ipv4Packet);
            Assert.Equal(MacAddress.Parse(smac),tx.Frame.SourceMac); Assert.Equal(MacAddress.Parse(dmac),tx.Frame.DestinationMac);
            var packet=Assert.IsType<Ipv4Packet>(tx.Frame.Payload); Assert.Equal(Ip(source),packet.SourceIp); Assert.Equal(Ip(destination),packet.DestinationIp); Assert.Equal(ttl,packet.Ttl);
            Assert.Equal((ushort)1,packet.Payload.Identifier); Assert.Equal((ushort)1,packet.Payload.Sequence);
            Assert.Equal(source=="192.168.1.10" ? IcmpKind.EchoRequest : IcmpKind.EchoReply,packet.Payload.Kind);
        }
        Assert.Equal(MacAddress.Parse("00:00:00:00:01:fe"),e.ArpEntry("host1",Ip("192.168.1.1")));
        Assert.Equal(MacAddress.Parse("00:00:00:00:02:fe"),e.ArpEntry("host2",Ip("10.0.0.1")));
        Assert.Null(e.ArpEntry("host1",Ip("10.0.0.20"))); Assert.Null(e.ArpEntry("host2",Ip("192.168.1.10")));
        Assert.Equal(MacAddress.Parse("00:00:00:00:01:01"),e.RouterArpEntry(Endpoint.Of("r1","eth0"),Ip("192.168.1.10")));
        Assert.Equal(MacAddress.Parse("00:00:00:00:02:02"),e.RouterArpEntry(Endpoint.Of("r1","eth1"),Ip("10.0.0.20")));
        foreach (var (sw,mac,port) in new[] { ("sw1","00:00:00:00:01:01","port1"),("sw1","00:00:00:00:01:fe","port2"),("sw2","00:00:00:00:02:fe","port1"),("sw2","00:00:00:00:02:02","port2") })
        { Assert.Equal(port,e.MacPort(sw,MacAddress.Parse(mac))); Assert.Equal(2,e.MacTableEntryCount(sw)); }
    }

    [Fact]
    public void ConcurrentRemotePingsShareResolutionAndLeaveRouterFifoAtTtl63()
    {
        var e=S1Fixture(); foreach (ushort id in new ushort[] {3,1,2}) e.SendPing("host1",Ip("10.0.0.20"),id,1);
        e.RunUntilQuiescent(1000);
        var forwarded=e.Trace.Records.OfType<FrameTransmitted>().Where(f => f.From==Endpoint.Of("r1","eth1") && f.Frame.Payload is Ipv4Packet).Select(f => (Ipv4Packet)f.Frame.Payload).ToArray();
        Assert.Equal(new ushort[] {3,1,2},forwarded.Select(p=>p.Payload.Identifier)); Assert.All(forwarded,p=>Assert.Equal(63,p.Ttl));
        Assert.Equal(new ushort[] {3,1,2},e.Trace.Records.OfType<PingCompleted>().Select(p=>p.Identifier));
        Assert.All(e.Trace.Records.OfType<PingCompleted>(),p=>Assert.Equal("host1",p.Host));
        foreach (var ep in new[] {Endpoint.Of("host1","eth0"),Endpoint.Of("r1","eth1")}) Assert.Single(e.Trace.Records.OfType<FrameTransmitted>(),f => f.From==ep && f.Frame.Payload is ArpPacket { Operation:ArpOperation.Request });
        Assert.Equal(17500UL,e.CurrentTimeNs);
    }

    [Fact]
    public void ReverseWarmPingUsesCachedGatewayWithoutRefreshingRouterArpLifetime()
    {
        var e=S1Fixture(); e.SendPing("host1",Ip("10.0.0.20"),1,1); e.RunUntilQuiescent(1000); var before=e.Trace.NewestAvailableSequence!.Value;
        e.SendPing("host2",Ip("192.168.1.10"),7,1); e.RunUntilQuiescent(1000);
        var completed=Assert.Single(e.Trace.Records.OfType<PingCompleted>(),p=>p.Identifier==7); Assert.Equal("host2",completed.Host); Assert.Equal(8700UL,completed.RttNs); Assert.Equal(26200UL,e.CurrentTimeNs);
        Assert.DoesNotContain(e.Trace.Records.Where(r=>r.TraceSequence>before),r => r is TableUpdated { Update:ArpLearned } || r is FrameTransmitted { Frame.Payload:ArpPacket });
        e.AdvanceUntil(60_000_002_100,1000);
        Assert.Null(e.RouterArpEntry(Endpoint.Of("r1","eth0"),Ip("192.168.1.10"))); Assert.NotNull(e.RouterArpEntry(Endpoint.Of("r1","eth1"),Ip("10.0.0.20")));
        Assert.Contains(e.Trace.Records.OfType<TableUpdated>(),r=>r.AtNs==60_000_002_100 && r.Device=="r1" && r.ObservationInterface=="eth0" && r.Update is ArpEntryExpired a && a.Ip==Ip("192.168.1.10"));
    }

    [Fact]
    public void FailedGatewayArpClosesRemotePingNotGatewayKey()
    {
        var e=new SimulationEngine(); Host(e,"h","192.168.1.10","192.168.1.1","00:00:00:00:01:01");
        e.CreateL2Switch("sw",["p"]); e.ConnectLink("wire",Endpoint.Of("h","eth0"),Endpoint.Of("sw","p"));
        e.SendPing("h",Ip("10.0.0.20"),4,1); e.RunUntilQuiescent(1000);
        Assert.Equal(3_000_000_000UL,e.CurrentTimeNs);
        var failed=Assert.Single(e.Trace.Records.OfType<PingFailed>()); Assert.Equal("h",failed.Host); Assert.Equal(Ip("10.0.0.20"),failed.TargetIp); Assert.Equal((ushort)4,failed.Identifier); Assert.Equal(PingFailureReason.ArpTimeout,failed.Reason);
        e.ConfigureHostIpv4(Endpoint.Of("h","eth0"),MacAddress.Parse("00:00:00:00:01:01"),new(new(Ip("192.168.1.11"),24)),true);
    }

    [Theory]
    [InlineData("192.168.1.1")]
    [InlineData("10.0.0.1")]
    public void RouterLocalEchoUsesRequestedIpNotEgressIp(string target)
    {
        var e=new SimulationEngine(); Host(e,"h","192.168.1.10","192.168.1.1","00:00:00:00:01:01"); Router(e);
        e.ConnectLink("wire",Endpoint.Of("h","eth0"),Endpoint.Of("r1","eth0")); e.SendPing("h",Ip(target),9,1); e.RunUntilQuiescent(100);
        var completed=Assert.Single(e.Trace.Records.OfType<PingCompleted>()); Assert.Equal("h",completed.Host); Assert.Equal(4300UL,completed.RttNs);
        var reply=Assert.Single(e.Trace.Records.OfType<FrameTransmitted>(),f=>f.From==Endpoint.Of("r1","eth0") && f.Frame.Payload is Ipv4Packet);
        var packet=Assert.IsType<Ipv4Packet>(reply.Frame.Payload); Assert.Equal(Ip(target),packet.SourceIp); Assert.Equal(Ip("192.168.1.10"),packet.DestinationIp); Assert.Equal(64,packet.Ttl); Assert.Equal(MacAddress.Parse("00:00:00:00:01:fe"),reply.Frame.SourceMac);
        Assert.DoesNotContain(e.Trace.Records,r=>r is PingCompleted { Host:"r1" } || r is PingFailed { Host:"r1" });
    }

    [Fact]
    public void StaticHostRouteArpsExplicitNextHopNotFinalDestination()
    {
        var e=S1Fixture(); e.AddRouterStaticRoute("r1",new(Ip("10.0.0.99"),32),Ip("10.0.0.20"),"eth1",999);
        e.SendPing("host1",Ip("10.0.0.99"),1,1); e.AdvanceUntil(11000,1000);
        Assert.Contains(e.Trace.Records.OfType<ArpPacketQueued>(),q=>q.Device=="r1" && q.TargetIp==Ip("10.0.0.20") && q.Packet.DestinationIp==Ip("10.0.0.99") && q.Packet.Ttl==63);
        Assert.Contains(e.Trace.Records.OfType<FrameTransmitted>(),f=>f.From==Endpoint.Of("r1","eth1") && f.Frame.Payload is Ipv4Packet p && p.DestinationIp==Ip("10.0.0.99") && p.Ttl==63 && f.Frame.DestinationMac==MacAddress.Parse("00:00:00:00:02:02"));
        Assert.DoesNotContain(e.Trace.Records.OfType<FrameTransmitted>(),f=>f.From.Device=="r1" && f.Frame.Payload is ArpPacket a && a.TargetIp==Ip("10.0.0.99"));
    }

    [Fact]
    public void IdenticalNeighborIpOnTwoInterfacesHasIndependentArpEntries()
    {
        var e=new SimulationEngine(); e.CreateRouter("r",[new("a",MacAddress.Parse("02:00:00:00:01:fe"),new(Ip("192.168.1.1"),24),true),new("b",MacAddress.Parse("02:00:00:00:02:fe"),new(Ip("192.168.1.129"),25),true)]);
        foreach(var(name,port,prefix,mac)in new[]{("h1","a",24,"02:00:00:00:01:01"),("h2","b",25,"02:00:00:00:02:02")})
        {
            e.CreateHost(name,["p"]);e.ConfigureHostIpv4(Endpoint.Of(name,"p"),MacAddress.Parse(mac),new(new(Ip("192.168.1.130"),prefix)),true);
            e.ConnectLink(name,Endpoint.Of(name,"p"),Endpoint.Of("r",port));
        }
        e.SendPing("h1",Ip("192.168.1.1"),1,1);e.SendPing("h2",Ip("192.168.1.129"),2,1);
        e.AdvanceUntil(2200,100);
        Assert.Equal(MacAddress.Parse("02:00:00:00:01:01"),e.RouterArpEntry(Endpoint.Of("r","a"),Ip("192.168.1.130")));
        Assert.Equal(MacAddress.Parse("02:00:00:00:02:02"),e.RouterArpEntry(Endpoint.Of("r","b"),Ip("192.168.1.130")));
    }

    [Fact]
    public void UnsupportedRemoteCommandWithoutGatewayLeavesNoHiddenSession()
    {
        var e=new SimulationEngine(); Host(e,"h","192.168.1.10",null,"00:00:00:00:01:01");
        Assert.Throws<InvalidOperationException>(()=>e.SendPing("h",Ip("10.0.0.20"),1,1)); Assert.Empty(e.Trace.Records);
        e.ConfigureHostIpv4(Endpoint.Of("h","eth0"),MacAddress.Parse("00:00:00:00:01:01"),new(new(Ip("192.168.1.10"),24),Ip("192.168.1.1")),true);
    }

    [Fact]
    public void RouterRetryAfterOneSecondPreservesQueuedTtlAndOriginalRtt()
    {
        var e=S1Fixture();e.SetInterfaceAdminState(Endpoint.Of("host2","eth0"),false);e.SendPing("host1",Ip("10.0.0.20"),1,1);e.AdvanceUntil(1_000_006_599,1000);
        e.SetInterfaceAdminState(Endpoint.Of("host2","eth0"),true);e.RunUntilQuiescent(1000);Assert.Equal(1_000_017_500UL,e.CurrentTimeNs);
        Assert.Equal(new ulong[]{6600,1_000_006_600},e.Trace.Records.OfType<FrameTransmitted>().Where(f=>f.From==Endpoint.Of("r1","eth1") && f.Frame.Payload is ArpPacket {Operation:ArpOperation.Request}).Select(f=>f.AtNs));
        var done=Assert.Single(e.Trace.Records.OfType<PingCompleted>());Assert.Equal("host1",done.Host);Assert.Equal(1_000_017_500UL,done.RttNs);
        Assert.Contains(e.Trace.Records.OfType<FrameTransmitted>(),f=>f.From==Endpoint.Of("r1","eth1") && f.Frame.Payload is Ipv4Packet {Ttl:63});
    }

    [Fact]
    public void ResolvedRouterRetryIsObsoleteAndConfigurationRemainsIdle()
    {
        var e=S1Fixture();e.SendPing("host1",Ip("10.0.0.20"),1,1);e.RunUntilQuiescent(1000);var before=e.Trace.Records.ToArray();
        e.AdvanceUntil(1_000_006_600,1000);Assert.Equal(before,e.Trace.Records);
        e.AddRouterStaticRoute("r1",new(Ip("203.0.113.0"),24),null,"eth0",1);Assert.Equal(before,e.Trace.Records);
    }
}
