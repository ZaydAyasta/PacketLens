using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class E21RouterAdmissionTests(Xunit.Abstractions.ITestOutputHelper output)
{
    private static Ipv4Address Ip(string text) => Ipv4Address.Parse(text);
    private static MacAddress Mac(string text) => MacAddress.Parse(text);
    private static Endpoint Ingress => Endpoint.Of("r1","eth0");
    private static SimulationEngine Fixture()
    {
        var e=new SimulationEngine();
        e.CreateHost("host1",["eth0"]);
        e.ConfigureHostIpv4(Endpoint.Of("host1","eth0"),Mac("02:00:00:00:01:01"),new(new(Ip("192.168.1.10"),24),Ip("192.168.1.1")),true);
        e.CreateRouter("r1",[new("eth0",Mac("02:00:00:00:02:01"),new(Ip("192.168.1.1"),24),true),new("eth1",Mac("02:00:00:00:02:02"),new(Ip("203.0.113.1"),24),false)]);
        e.ConnectLink("l1",Endpoint.Of("host1","eth0"),Ingress);
        return e;
    }
    private static Ipv4Packet Request() => new(Ip("192.168.1.10"),Ip("198.51.100.1"),64,new IcmpPacket(IcmpKind.EchoRequest,7,1));
    private static EthernetFrame Frame(Ipv4Packet packet,string destination="02:00:00:00:02:01") => new(Mac("02:00:00:00:01:01"),Mac(destination),packet);
    private static void AssertSingleIpv4Drop(SimulationEngine e,Ipv4Packet original,string reason)
    {
        Assert.Equal(2,e.Trace.Records.Count);
        Assert.Equal(Ingress,Assert.IsType<FrameDelivered>(e.Trace.Records[0]).AtEndpoint);
        Assert.Equal(original with {Ttl=reason=="NoRouteToHost"?(byte)(original.Ttl+1):original.Ttl},Assert.IsType<FrameDelivered>(e.Trace.Records[0]).Frame.Payload);
        var drop=Assert.IsType<PacketDropped>(e.Trace.Records[1]);
        Assert.Equal("r1",drop.Device);Assert.Equal("eth0",drop.InterfaceName);
        Assert.Null(drop.EthernetFrame);Assert.Equal(original,drop.Ipv4Packet);Assert.Equal(reason,drop.Reason.ToString());
        Assert.Equal(new ulong[]{1,2},e.Trace.Records.Select(r=>r.TraceSequence));
        Assert.All(e.Trace.Records,r=>Assert.Equal(0UL,r.AtNs));Assert.Equal(0UL,e.CurrentTimeNs);
        Assert.Equal(2UL,e.Trace.TotalEmittedEver);Assert.Equal(0UL,e.Trace.DroppedByCapacity);
    }
    [Fact]
    public void MulticastL2UnicastIpIsIpv4PolicyDrop()
    {
      foreach(var destination in new[]{"01:00:5e:00:00:01","ff:ff:ff:ff:ff:ff"}) {
        var e=S8Fixture();var p=Request();e.InjectFrameForTesting(Ingress,Frame(p,destination),0);
        Assert.Equal(ExecutionOutcome.Quiescent,e.RunUntilQuiescent(1000).Outcome);
        AssertSingleIpv4Drop(e,p,"UnsupportedIpDestination");
      }
    }
    [Theory]
    [InlineData("255.255.255.255")][InlineData("192.168.1.255")][InlineData("203.0.113.255")]
    [InlineData("224.0.0.1")][InlineData("239.255.255.255")]
    public void S8CIpBroadcastMulticastRejectedBeforeTtlAndLocalService(string destination)
    {
        foreach (byte ttl in new byte[]{0,1,64})
        {
            var e=S8Fixture();var p=Request() with {DestinationIp=Ip(destination),Ttl=ttl};
            e.InjectFrameForTesting(Ingress,Frame(p),0);e.RunUntilQuiescent(1000);
            AssertSingleIpv4Drop(e,p,"UnsupportedIpDestination");
        }
    }
    [Theory]
    [InlineData("0.0.0.0")][InlineData("0.1.2.3")][InlineData("127.0.0.1")][InlineData("224.0.0.1")]
    [InlineData("255.255.255.255")][InlineData("192.168.1.255")][InlineData("203.0.113.255")]
    [InlineData("0.255.255.255")][InlineData("127.255.255.254")][InlineData("239.255.255.255")]
    public void S8DInvalidSourceRejectedBeforeEchoAndTransit(string source)
    {
        foreach(var destination in new[]{"198.51.100.1","192.168.1.1","203.0.113.1"})
        foreach(byte ttl in new byte[]{0,1,64})
        {
            var e=S8Fixture();var p=Request() with {SourceIp=Ip(source),DestinationIp=Ip(destination),Ttl=ttl};
            e.InjectFrameForTesting(Ingress,Frame(p),0);e.RunUntilQuiescent(1000);
            AssertSingleIpv4Drop(e,p,"InvalidIpv4Source");
        }
    }
    private static SimulationEngine S8Fixture()
    {
        var e=Fixture();e.SeedRouterArpForTesting(Ingress,Ip("192.168.1.10"),Mac("02:00:00:00:01:01"));return e;
    }
    private static IcmpErrorMessage Error(int kind)
    {
        var original=OriginalDatagramSummary.From(Request());
        return kind switch {0=>new TimeExceededMessage(original),1=>new DestinationNetworkUnreachableMessage(original),2=>new DestinationHostUnreachableMessage(original),_=>throw new ArgumentOutOfRangeException(nameof(kind))};
    }
    [Fact]
    public void ReceivedHostUnreachableHasCodeOne()
    {
        var message=Error(2);Assert.Equal((byte)3,message.Type);Assert.Equal((byte)1,message.Code);
        Assert.Equal(IcmpKind.DestinationHostUnreachable,message.Kind);Assert.Equal(OriginalDatagramSummary.From(Request()),message.Original);
        var summary=OriginalDatagramSummary.From(Request() with {Payload=message});
        Assert.Null(summary.EchoIdentifier);Assert.Null(summary.EchoSequence);
    }
    [Theory]
    [InlineData(0)][InlineData(1)][InlineData(64)]
    public void S8AEligibleGeneratesExactlyOneErrorAndRealDeliveryWithoutPing(byte ttl)
    {
        var e=S8Fixture();var p=Request() with {Ttl=ttl};e.InjectFrameForTesting(Ingress,Frame(p),0);
        Assert.Equal(ExecutionOutcome.Quiescent,e.RunUntilQuiescent(1000).Outcome);
        Assert.Equal(4,e.Trace.Records.Count);Assert.Equal(1100UL,e.CurrentTimeNs);
        Assert.Equal(Frame(p),Assert.IsType<FrameDelivered>(e.Trace.Records[0]).Frame);
        var drop=Assert.IsType<PacketDropped>(e.Trace.Records[1]);Assert.Equal("r1",drop.Device);Assert.Equal("eth0",drop.InterfaceName);
        Assert.Equal(p with {Ttl=ttl>1 ? (byte)(ttl-1):ttl},drop.Ipv4Packet);Assert.Null(drop.EthernetFrame);
        Assert.Equal(ttl>1 ? DropReason.NoRouteToHost:DropReason.TtlExpiredInTransit,drop.Reason);
        var response=new Ipv4Packet(Ip("192.168.1.1"),p.SourceIp,64,Error(ttl>1 ? 1:0));
        var expected=new EthernetFrame(Mac("02:00:00:00:02:01"),Mac("02:00:00:00:01:01"),response);
        var tx=Assert.IsType<FrameTransmitted>(e.Trace.Records[2]);Assert.Equal(Ingress,tx.From);Assert.Equal(Endpoint.Of("host1","eth0"),tx.To);Assert.Equal(expected,tx.Frame);
        var rx=Assert.IsType<FrameDelivered>(e.Trace.Records[3]);Assert.Equal(Endpoint.Of("host1","eth0"),rx.AtEndpoint);Assert.Equal(expected,rx.Frame);
        Assert.Equal(new ulong[]{1,2,3,4},e.Trace.Records.Select(r=>r.TraceSequence));Assert.Equal(new ulong[]{0,0,100,1100},e.Trace.Records.Select(r=>r.AtNs));
    }
    [Theory]
    [InlineData(0)][InlineData(1)][InlineData(2)]
    public void S8BAllErrorInputsNeverRecurse(int kind)
    {
        foreach(byte ttl in new byte[]{0,1,64})
        {
            var e=S8Fixture();var p=Request() with {Ttl=ttl,Payload=Error(kind)};
            e.InjectFrameForTesting(Ingress,Frame(p),0);e.RunUntilQuiescent(1000);
            AssertSingleIpv4Drop(e,p with {Ttl=ttl>1 ? (byte)(ttl-1):ttl},ttl>1 ? "NoRouteToHost":"TtlExpiredInTransit");
        }
    }
    [Fact]
    public void PreseedHasNormalEpochExpiryWithoutFakeLearning()
    {
        var e=S8Fixture();Assert.Empty(e.Trace.Records);
        e.AdvanceUntil(SimulationEngine.ArpCacheTtlNs-1,1000);Assert.Equal(Mac("02:00:00:00:01:01"),e.RouterArpEntry(Ingress,Ip("192.168.1.10")));
        e.AdvanceUntil(SimulationEngine.ArpCacheTtlNs,1000);Assert.Null(e.RouterArpEntry(Ingress,Ip("192.168.1.10")));
        var row=Assert.IsType<TableUpdated>(Assert.Single(e.Trace.Records));Assert.Equal("r1",row.Device);Assert.Equal("eth0",row.ObservationInterface);
        Assert.Equal(new ArpEntryExpired(Ip("192.168.1.10")),row.Update);Assert.Equal(SimulationEngine.ArpCacheTtlNs,row.AtNs);
    }
    [Fact]
    public void ForeignUnicastMacIsEthernetDropEvenForForbiddenIp()
    {
        foreach(var destination in new[]{"198.51.100.1","224.0.0.1","192.168.1.1"})
        {
            var e=S8Fixture();var input=Frame(Request() with {DestinationIp=Ip(destination)},"02:00:00:00:63:01");
            e.InjectFrameForTesting(Ingress,input,0);e.RunUntilQuiescent(1000);
            Assert.Equal(2,e.Trace.Records.Count);var drop=Assert.IsType<PacketDropped>(e.Trace.Records[1]);
            Assert.Equal(DropReason.DestinationMacMismatch,drop.Reason);Assert.Equal(input,drop.EthernetFrame);Assert.Null(drop.Ipv4Packet);Assert.Equal(0UL,e.CurrentTimeNs);
        }
    }
    [Fact]
    public void ValidLocalEchoBeforeTtlZeroOneSixtyFour()
    {
        foreach(var destination in new[]{"192.168.1.1","203.0.113.1"})foreach(byte ttl in new byte[]{0,1,64})
        {
            var e=S8Fixture();e.InjectFrameForTesting(Ingress,Frame(Request() with {DestinationIp=Ip(destination),Ttl=ttl}),0);e.RunUntilQuiescent(1000);
            Assert.Equal(3,e.Trace.Records.Count);var tx=Assert.IsType<FrameTransmitted>(e.Trace.Records[1]);
            Assert.Equal(new Ipv4Packet(Ip(destination),Ip("192.168.1.10"),64,new IcmpPacket(IcmpKind.EchoReply,7,1)),tx.Frame.Payload);
            Assert.Equal(1100UL,e.CurrentTimeNs);
        }
    }
    private static SimulationEngine ArpTransitFixture()
    {
        var e=Fixture();e.SetInterfaceAdminState(Endpoint.Of("r1","eth1"),true);
        e.CreateRouter("r2",[new("eth0",Mac("02:00:00:00:03:01"),new(Ip("203.0.113.2"),24),true),new("eth1",Mac("02:00:00:00:03:02"),new(Ip("10.0.0.1"),24),false)]);
        e.ConnectLink("l2",Endpoint.Of("r1","eth1"),Endpoint.Of("r2","eth0"));
        e.AddRouterStaticRoute("r1",new(Ip("198.51.100.0"),24),Ip("203.0.113.2"),"eth1",10);
        e.AddRouterStaticRoute("r2",new(Ip("192.168.1.0"),24),Ip("203.0.113.1"),"eth0",10);return e;
    }
    private static SimulationEngine PostArpCase(int kind,byte ttl,bool fail)
    {
        var e=ArpTransitFixture();if(fail)e.SetInterfaceAdminState(Endpoint.Of("r2","eth0"),false);
        var input=Request() with {Ttl=ttl,Payload=Error(kind)};e.InjectFrameForTesting(Ingress,Frame(input),0);e.StepEvents(1);
        var queue=Assert.Single(e.Trace.Records.OfType<ArpPacketQueued>());var forwarded=input with {Ttl=(byte)(ttl-1)};
        Assert.Equal(forwarded,queue.Packet);Assert.Equal("eth1",queue.ObservationInterface);Assert.Equal(Ip("203.0.113.2"),queue.TargetIp);
        Assert.Equal(Frame(input),Assert.IsType<FrameDelivered>(e.Trace.Records[0]).Frame);
        Assert.Equal(ExecutionOutcome.Quiescent,e.RunUntilQuiescent(1000).Outcome);
        var drop=Assert.Single(e.Trace.Records.OfType<PacketDropped>(),d=>d.Ipv4Packet is not null);
        var tx=e.Trace.Records.OfType<FrameTransmitted>().Where(t=>t.Frame.Payload is Ipv4Packet).ToArray();
        if(fail){Assert.Empty(tx);Assert.Equal(3_000_000_100UL,drop.AtNs);Assert.Equal(DropReason.ArpResolutionFailed,drop.Reason);Assert.Equal(forwarded,drop.Ipv4Packet);}
        else{Assert.Equal(forwarded,Assert.Single(tx).Frame.Payload);Assert.Equal(3300UL,drop.AtNs);Assert.Equal(ttl==2?DropReason.TtlExpiredInTransit:DropReason.NoRouteToHost,drop.Reason);Assert.Equal(forwarded with {Ttl=ttl==2?(byte)1:(byte)(ttl-2)},drop.Ipv4Packet);}
        Assert.Empty(e.Trace.Records.OfType<PingCompleted>());Assert.Empty(e.Trace.Records.OfType<PingFailed>());return e;
    }
    [Fact]
    public void OriginalErrorAndContextSurviveArpAndNeverGenerateSecondaryError()
    {
        for(var kind=0;kind<3;kind++){foreach(byte ttl in new byte[]{2,64})PostArpCase(kind,ttl,false);PostArpCase(kind,64,true);}
    }
    [Fact]
    public void NoReturnAndSuppressionDoNotDependOnNeighborCache()
    {
        foreach(var warm in new[]{false,true})foreach(byte ttl in new byte[]{0,1,64})for(var kind=0;kind<4;kind++)
        {
            var e=warm?S8Fixture():Fixture();var p=Request() with {SourceIp=Ip("198.18.0.10"),Ttl=ttl};if(kind<3)p=p with {Payload=Error(kind)};
            e.InjectFrameForTesting(Ingress,Frame(p),0);e.RunUntilQuiescent(1000);
            AssertSingleIpv4Drop(e,p with {Ttl=ttl>1?(byte)(ttl-1):ttl},ttl>1?"NoRouteToHost":"TtlExpiredInTransit");
        }
    }
    [Fact]
    public void RemoteDot255IsNotInventedAsBroadcastAndMulticastBoundsAreExact()
    {
        foreach(var destination in new[]{"198.51.100.255","223.255.255.255","240.0.0.1"})
        {
            var e=S8Fixture();e.InjectFrameForTesting(Ingress,Frame(Request() with {DestinationIp=Ip(destination)}),0);e.RunUntilQuiescent(1000);
            Assert.Equal(4,e.Trace.Records.Count);Assert.Equal(DropReason.NoRouteToHost,Assert.IsType<PacketDropped>(e.Trace.Records[1]).Reason);
        }
    }
    [Theory]
    [InlineData(31)][InlineData(32)]
    public void Slash31AndSlash32AddressesAreNotDirectedBroadcasts(byte prefix)
    {
        var e=new SimulationEngine();e.CreateHost("host1",["eth0"]);var mode=prefix==31?Ipv4LinkMode.PointToPoint:Ipv4LinkMode.MultiAccess;
        e.SetInterfaceIpv4Mode(Endpoint.Of("host1","eth0"),mode);
        e.ConfigureHostIpv4(Endpoint.Of("host1","eth0"),Mac("02:00:00:00:01:01"),new(new(Ip("172.16.0.0"),prefix)),true);
        e.CreateRouter("r1",[new("eth0",Mac("02:00:00:00:02:01"),new(Ip("172.16.0.1"),prefix),true,mode),new("eth1",Mac("02:00:00:00:02:02"),new(Ip("203.0.113.1"),24),false)]);
        e.ConnectLink("l1",Endpoint.Of("host1","eth0"),Ingress);
        if(prefix==32)e.AddRouterStaticRoute("r1",new(Ip("172.16.0.0"),32),null,"eth0",1);
        e.ValidateIpv4Topology();e.InjectFrameForTesting(Ingress,Frame(Request() with {SourceIp=Ip("172.16.0.0"),DestinationIp=Ip("172.16.0.1"),Ttl=0}),0);e.RunUntilQuiescent(1000);
        Assert.Empty(e.Trace.Records.OfType<PacketDropped>());
        var tx=Assert.Single(e.Trace.Records.OfType<FrameTransmitted>(),t=>t.Frame.Payload is Ipv4Packet);
        Assert.Equal(new Ipv4Packet(Ip("172.16.0.1"),Ip("172.16.0.0"),64,new IcmpPacket(IcmpKind.EchoReply,7,1)),tx.Frame.Payload);
    }
    [Fact]
    public void FailedIngressIsNotReportedAsIpv4Admission()
    {
        foreach(var stale in new[]{false,true})
        {
            var e=S8Fixture();var input=Frame(Request());e.InjectFrameForTesting(Ingress,input,1000);
            if(stale){e.DisconnectLink("l1");e.ConnectLink("replacement",Endpoint.Of("host1","eth0"),Ingress);}else e.SetInterfaceAdminState(Ingress,false);
            e.RunUntilQuiescent(1000);var drop=Assert.IsType<PacketDropped>(Assert.Single(e.Trace.Records));
            Assert.Equal(input,drop.EthernetFrame);Assert.Null(drop.Ipv4Packet);Assert.Equal(stale?DropReason.LinkDisconnectedInFlight:DropReason.IngressInterfaceDown,drop.Reason);Assert.Equal(1000UL,e.CurrentTimeNs);
        }
    }
    [Fact]
    public void SeedRenewalInvalidatesOldEpochWithoutIpv4Learning()
    {
        var e=S8Fixture();e.AdvanceUntil(10,1000);
        var arp=new EthernetFrame(Mac("02:00:00:00:01:01"),Mac("02:00:00:00:02:01"),new ArpPacket(ArpOperation.Reply,Mac("02:00:00:00:01:01"),Ip("192.168.1.10"),Mac("02:00:00:00:02:01"),Ip("192.168.1.1")));
        e.InjectFrameForTesting(Ingress,arp,10);e.RunUntilQuiescent(1000);var before=e.Trace.Records.ToArray();
        e.AdvanceUntil(SimulationEngine.ArpCacheTtlNs,1000);Assert.Equal(before,e.Trace.Records);Assert.Equal(Mac("02:00:00:00:01:01"),e.RouterArpEntry(Ingress,Ip("192.168.1.10")));
        e.AdvanceUntil(SimulationEngine.ArpCacheTtlNs+10,1000);Assert.Null(e.RouterArpEntry(Ingress,Ip("192.168.1.10")));Assert.Equal(3,e.Trace.Records.Count);
    }
    [Fact]
    public void LegacyErrorTagsCannotEscapeSuppressionOrRecurseAfterArp()
    {
        foreach(var kind in new[]{IcmpKind.TimeExceeded,IcmpKind.DestinationHostUnreachable})foreach(var pending in new[]{false,true})
        {
            var e=pending?ArpTransitFixture():S8Fixture();if(pending)e.SetInterfaceAdminState(Endpoint.Of("r2","eth0"),false);
            var p=Request() with {Payload=new IcmpPacket(kind,7,1)};e.InjectFrameForTesting(Ingress,Frame(p),0);e.RunUntilQuiescent(1000);
            var dropped=p with {Ttl=63};
            if(pending){Assert.DoesNotContain(e.Trace.Records.OfType<FrameTransmitted>(),t=>t.Frame.Payload is Ipv4Packet);var drop=Assert.Single(e.Trace.Records.OfType<PacketDropped>(),d=>d.Ipv4Packet is not null);Assert.Equal(DropReason.ArpResolutionFailed,drop.Reason);Assert.Equal(dropped,drop.Ipv4Packet);}
            else AssertSingleIpv4Drop(e,dropped,"NoRouteToHost");
        }
    }
    [Fact]
    public void NearestValidSourcePrefixesAreNotRejectedOrLearnedFromIpv4()
    {
        foreach(var source in new[]{"1.0.0.1","126.255.255.254","128.0.0.1","223.255.255.254"})
        {
            var e=S8Fixture();var p=Request() with {SourceIp=Ip(source)};e.InjectFrameForTesting(Ingress,Frame(p),0);e.RunUntilQuiescent(1000);
            AssertSingleIpv4Drop(e,p with {Ttl=63},"NoRouteToHost");Assert.Null(e.RouterArpEntry(Ingress,Ip(source)));
        }
    }
    private static IEnumerable<string> Cases()
    {
        foreach(var ttl in new[]{0,1,64})
        {
            yield return $"A:{ttl}";
            for(var kind=0;kind<3;kind++)for(var missing=0;missing<2;missing++)yield return $"B:{kind}:{ttl}:{missing}";
            for(var dst=0;dst<5;dst++)yield return $"C:{dst}:{ttl}";
            for(var group=0;group<2;group++)yield return $"L:{group}:{ttl}";
            for(var src=0;src<10;src++)for(var dst=0;dst<3;dst++)for(var route=0;route<2;route++)yield return $"D:{src}:{dst}:{ttl}:{route}";
            for(var warm=0;warm<2;warm++)yield return $"N:{ttl}:{warm}";
            for(var dst=0;dst<2;dst++)yield return $"E:{dst}:{ttl}";
        }
        for(var kind=0;kind<3;kind++){foreach(var ttl in new[]{2,64})yield return $"P:{kind}:{ttl}:0";yield return $"P:{kind}:64:1";}
        yield return "O:0";yield return "O:1";
    }
    private static SimulationEngine OrderedCase(bool reverse)
    {
        var e=S8Fixture();var inputs=new List<Ipv4Packet>{Request() with {Payload=Error(0)},Request() with {Payload=Error(1)},Request() with {SourceIp=Ip("0.1.2.3")}};
        if(reverse)inputs.Reverse();foreach(var p in inputs)e.InjectFrameForTesting(Ingress,Frame(p),0);
        e.RunUntilQuiescent(1000);Assert.Equal(6,e.Trace.Records.Count);
        for(var index=0;index<3;index++)
        {
            var p=inputs[index];Assert.Equal(Frame(p),Assert.IsType<FrameDelivered>(e.Trace.Records[index*2]).Frame);
            var invalid=p.SourceIp==Ip("0.1.2.3");var drop=Assert.IsType<PacketDropped>(e.Trace.Records[index*2+1]);
            Assert.Equal(invalid?DropReason.InvalidIpv4Source:DropReason.NoRouteToHost,drop.Reason);Assert.Equal(p with {Ttl=invalid?(byte)64:(byte)63},drop.Ipv4Packet);
        }
        Assert.Equal(new ulong[]{1,2,3,4,5,6},e.Trace.Records.Select(r=>r.TraceSequence));return e;
    }
    private static SimulationEngine ExecuteCase(string label)
    {
        var parts=label.Split(':');byte N(int i)=>byte.Parse(parts[i]);
        if(parts[0]=="P")return PostArpCase(N(1),N(2),N(3)==1);
        if(parts[0]=="O")return OrderedCase(N(1)==1);
        var e=parts[0]=="N" && N(2)==0?Fixture():S8Fixture();var p=Request();var mac="02:00:00:00:02:01";string? reason=null;
        switch(parts[0])
        {
            case "A":p=p with {Ttl=N(1)};break;
            case "B":p=p with {Payload=Error(N(1)),Ttl=N(2),SourceIp=N(3)==1?Ip("198.18.0.10"):p.SourceIp};reason=p.Ttl<=1?"TtlExpiredInTransit":"NoRouteToHost";break;
            case "C":p=p with {DestinationIp=Ip(new[]{"255.255.255.255","192.168.1.255","203.0.113.255","224.0.0.1","239.255.255.255"}[N(1)]),Ttl=N(2)};reason="UnsupportedIpDestination";break;
            case "L":mac=N(1)==0?"ff:ff:ff:ff:ff:ff":"01:00:5e:00:00:01";p=p with {Ttl=N(2)};reason="UnsupportedIpDestination";break;
            case "D":
                p=p with {SourceIp=Ip(new[]{"0.0.0.0","0.1.2.3","127.0.0.1","224.0.0.1","255.255.255.255","192.168.1.255","203.0.113.255","0.255.255.255","127.255.255.254","239.255.255.255"}[N(1)]),DestinationIp=Ip(new[]{"198.51.100.1","192.168.1.1","203.0.113.1"}[N(2)]),Ttl=N(3)};
                if(N(4)==1)e.AddRouterStaticRoute("r1",new(Ip("0.0.0.0"),0),null,"eth0",10);reason="InvalidIpv4Source";break;
            case "N":p=p with {SourceIp=Ip("198.18.0.10"),Ttl=N(1)};reason=p.Ttl<=1?"TtlExpiredInTransit":"NoRouteToHost";break;
            case "E":p=p with {DestinationIp=Ip(N(1)==0?"192.168.1.1":"203.0.113.1"),Ttl=N(2)};break;
            default:throw new ArgumentException("Unknown case",nameof(label));
        }
        e.InjectFrameForTesting(Ingress,Frame(p,mac),0);Assert.Equal(ExecutionOutcome.Quiescent,e.RunUntilQuiescent(1000).Outcome);
        if(reason is not null)AssertSingleIpv4Drop(e,p with {Ttl=reason=="NoRouteToHost"?(byte)(p.Ttl-1):p.Ttl},reason);
        else Assert.Equal(parts[0]=="A"?4:3,e.Trace.Records.Count);
        e.AddRouterStaticRoute("r1",new(Ip("10.123.0.1"),32),null,"eth0",1);return e;
    }
    private static string[] Projection(SimulationEngine e,string label)
    {
        static string Text(MacAddress? m)=>m is {} x?$"{x.A:x2}:{x.B:x2}:{x.C:x2}:{x.D:x2}:{x.E:x2}:{x.F:x2}":"-";
        var rows=E21RouterIcmpTests.ObservationProjection(e,label).Select(r=>r.Replace("ICMP4|","ADM5|")).ToList();
        rows.Add($"ADM5|{label}|state|{Text(e.ArpEntry("host1",Ip("192.168.1.1")))}|{Text(e.RouterArpEntry(Ingress,Ip("192.168.1.10")))}|{Text(e.RouterArpEntry(Endpoint.Of("r1","eth1"),Ip("203.0.113.2")))}");
        if(label.StartsWith("P:",StringComparison.Ordinal))rows.Add($"ADM5|{label}|state-r2|{Text(e.RouterArpEntry(Endpoint.Of("r2","eth0"),Ip("203.0.113.1")))}");
        return rows.ToArray();
    }
    [Fact]
    public void S8MatrixIsDeterministicAcrossOneHundredReconstructionsAndOrderVariants()
    {
        foreach(var label in Cases())
        {
            var expected=Projection(ExecuteCase(label),label);
            for(var i=1;i<100;i++)Assert.Equal(expected,Projection(ExecuteCase(label),label));
            foreach(var row in expected)output.WriteLine(row);
        }
    }
}
