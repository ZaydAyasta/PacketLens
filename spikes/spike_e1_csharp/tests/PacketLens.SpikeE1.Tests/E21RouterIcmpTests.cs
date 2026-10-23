using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class E21RouterIcmpTests(Xunit.Abstractions.ITestOutputHelper output)
{
    private static Ipv4Address Ip(string s) => Ipv4Address.Parse(s);
    private static MacAddress Mac(string s) => MacAddress.Parse(s);
    private static SimulationEngine Fixture()
    {
        var e = new SimulationEngine();
        e.CreateHost("h",["eth0"]);
        e.ConfigureHostIpv4(Endpoint.Of("h","eth0"),Mac("02:00:00:00:01:01"),new(new(Ip("192.168.1.10"),24),Ip("192.168.1.1")),true);
        e.CreateRouter("r",[new("a",Mac("02:00:00:00:02:01"),new(Ip("192.168.1.1"),24),true),new("b",Mac("02:00:00:00:02:02"),new(Ip("203.0.113.1"),24),false)]);
        e.ConnectLink("l1",Endpoint.Of("h","eth0"),Endpoint.Of("r","a")); return e;
    }
    [Theory]
    [InlineData(1,1,"198.51.100.1")]
    [InlineData(19,41,"10.99.8.7")]
    public void NoRouteGeneratesNetworkUnreachable(ushort id,ushort sequence,string destination)
    {
        var e=Fixture();e.SendPing("h",Ip(destination),id,sequence);
        Assert.Equal(ExecutionOutcome.Quiescent,e.RunUntilQuiescent(1000).Outcome);
        var tx=Assert.Single(e.Trace.Records.OfType<FrameTransmitted>(),t=>t.Frame.Payload is Ipv4Packet {Payload:IcmpErrorMessage});
        Assert.Equal(3300UL,tx.AtNs);Assert.Equal(Endpoint.Of("r","a"),tx.From);
        var packet=Assert.IsType<Ipv4Packet>(tx.Frame.Payload);
        Assert.Equal(Ip("192.168.1.1"),packet.SourceIp);Assert.Equal(Ip("192.168.1.10"),packet.DestinationIp);Assert.Equal(64,packet.Ttl);
        var error=Assert.IsType<DestinationNetworkUnreachableMessage>(packet.Payload);
        Assert.Equal((byte)3,error.Type);Assert.Equal((byte)0,error.Code);
        Assert.Equal(new OriginalDatagramSummary(Ip("192.168.1.10"),Ip(destination),1,id,sequence),error.Original);
        var drop=Assert.Single(e.Trace.Records.OfType<PacketDropped>());Assert.Equal(DropReason.NoRouteToHost,drop.Reason);Assert.Equal(63,drop.Ipv4Packet!.Ttl);
        Assert.Equal(3200UL,drop.AtNs);Assert.True(drop.TraceSequence<tx.TraceSequence);
        Assert.Contains(e.Trace.Records.OfType<FrameDelivered>(),r=>r.AtEndpoint.Device=="r" && r.Frame.Payload is Ipv4Packet {Ttl:64});
        Assert.Contains(e.Trace.Records.OfType<FrameDelivered>(),r=>r.AtEndpoint.Device=="h" && r.Frame.Payload is Ipv4Packet {Payload:IcmpErrorMessage});
        Assert.Empty(e.Trace.Records.OfType<PingCompleted>());Assert.Empty(e.Trace.Records.OfType<PingFailed>());Assert.Equal(4300UL,e.CurrentTimeNs);
    }

    [Theory]
    [InlineData(1)] [InlineData(64)]
    public void MissingReturnRouteKeepsOnlyOriginalDrop(byte ttl)
    {
        var e=Fixture();Inject(e,"8.8.8.8","198.51.100.1",ttl,7,2);e.RunUntilQuiescent(100);
        Assert.Equal(2,e.Trace.Records.Count);var drop=Assert.IsType<PacketDropped>(e.Trace.Records[1]);
        Assert.Equal(ttl==1 ? DropReason.TtlExpiredInTransit : DropReason.NoRouteToHost,drop.Reason);
        Assert.Empty(e.Trace.Records.OfType<FrameTransmitted>());Assert.Equal(0UL,e.CurrentTimeNs);
    }
    [Fact]
    public void GeneratedErrorUsesReturnLpmNextHopAndEgressIp()
    {
        var e=Fixture();e.SetInterfaceAdminState(Endpoint.Of("r","b"),true);e.CreateHost("peer",["p"]);
        e.ConfigureHostIpv4(Endpoint.Of("peer","p"),Mac("02:00:00:00:03:01"),new(new(Ip("203.0.113.20"),24)),true);
        e.ConnectLink("l2",Endpoint.Of("r","b"),Endpoint.Of("peer","p"));
        e.AddRouterStaticRoute("r",new(Ip("192.168.1.10"),32),Ip("203.0.113.20"),"b",99);
        Inject(e,"192.168.1.10","198.51.100.99",64,65535,101);e.RunUntilQuiescent(100);
        var tx=Assert.Single(e.Trace.Records.OfType<FrameTransmitted>(),t=>t.Frame.Payload is Ipv4Packet);
        Assert.Equal(Endpoint.Of("r","b"),tx.From);Assert.Equal(Mac("02:00:00:00:02:02"),tx.Frame.SourceMac);Assert.Equal(Mac("02:00:00:00:03:01"),tx.Frame.DestinationMac);
        var p=Assert.IsType<Ipv4Packet>(tx.Frame.Payload);Assert.Equal(Ip("203.0.113.1"),p.SourceIp);Assert.Equal(Ip("192.168.1.10"),p.DestinationIp);Assert.Equal(64,p.Ttl);
        Assert.Contains(e.Trace.Records.OfType<ArpPacketQueued>(),q=>q.TargetIp==Ip("203.0.113.20") && q.Packet.Payload is IcmpErrorMessage && q.Packet.Ttl==64);
        Assert.DoesNotContain(e.Trace.Records.OfType<FrameDelivered>(),r=>r.AtEndpoint.Device=="h" && r.Frame.Payload is Ipv4Packet {Payload:IcmpErrorMessage});
    }
    [Fact]
    public void GeneratedErrorEgressFailureIsDropNotDelivery()
    {
        var e=Fixture();e.SendPing("h",Ip("198.51.100.1"),1,1);e.AdvanceUntil(3200,100);e.SetInterfaceAdminState(Endpoint.Of("r","a"),false);e.RunUntilQuiescent(100);
        Assert.DoesNotContain(e.Trace.Records.OfType<FrameTransmitted>(),t=>t.Frame.Payload is Ipv4Packet {Payload:IcmpErrorMessage});
        Assert.Equal(2,e.Trace.Records.OfType<PacketDropped>().Count());
        var errorDrop=Assert.Single(e.Trace.Records.OfType<PacketDropped>(),d=>d.Reason==DropReason.EgressInterfaceDown);
        Assert.Equal(3300UL,errorDrop.AtNs);Assert.IsAssignableFrom<IcmpErrorMessage>(Assert.IsType<Ipv4Packet>(errorDrop.EthernetFrame!.Payload).Payload);
        Assert.Equal(3300UL,e.CurrentTimeNs);
    }
    [Theory]
    [InlineData(1)] [InlineData(64)]
    public void IcmpErrorCannotTriggerAnotherErrorInImplementedFailurePaths(byte ttl)
    {
        var original = new OriginalDatagramSummary(Ip("1.1.1.1"),Ip("2.2.2.2"),1,7,1);
        foreach (var error in new IcmpErrorMessage[] { new TimeExceededMessage(original), new DestinationNetworkUnreachableMessage(original) })
        {
            var e = Fixture();
            var packet = new Ipv4Packet(Ip("192.168.1.10"),Ip("198.51.100.1"),ttl,error);
            e.InjectFrameForTesting(Endpoint.Of("r","a"),new(Mac("02:00:00:00:01:01"),Mac("02:00:00:00:02:01"),packet),0);
            e.RunUntilQuiescent(100);
            Assert.Equal(2,e.Trace.Records.Count);
            var drop = Assert.IsType<PacketDropped>(e.Trace.Records[1]);
            Assert.Equal(error,Assert.IsType<Ipv4Packet>(drop.Ipv4Packet).Payload);
            Assert.Empty(e.Trace.Records.OfType<FrameTransmitted>());
            Assert.Equal(0UL,e.CurrentTimeNs);
        }
    }
    [Fact]
    public void GeneratedErrorArpFailureDropsItWithoutSecondaryError()
    {
        var e=Fixture();e.SetInterfaceAdminState(Endpoint.Of("h","eth0"),false);Inject(e,"192.168.1.10","198.51.100.1",1);e.RunUntilQuiescent(1000);
        Assert.DoesNotContain(e.Trace.Records.OfType<FrameTransmitted>(),t=>t.Frame.Payload is Ipv4Packet);
        var drop=Assert.Single(e.Trace.Records.OfType<PacketDropped>(),d=>d.Reason==DropReason.ArpResolutionFailed && d.Ipv4Packet?.Payload is IcmpErrorMessage);
        Assert.Equal(3_000_000_100UL,drop.AtNs);
        var dropped=drop.Ipv4Packet!;Assert.Equal(64,dropped.Ttl);
        Assert.Equal(new OriginalDatagramSummary(Ip("192.168.1.10"),Ip("198.51.100.1"),1,7,1),Assert.IsType<TimeExceededMessage>(dropped.Payload).Original);
        Assert.Equal(3,e.Trace.Records.OfType<FrameTransmitted>().Count(t=>t.Frame.Payload is ArpPacket {Operation:ArpOperation.Request}));
        e.AddRouterStaticRoute("r",new(Ip("10.0.0.0"),24),null,"a",1);
    }

    private static EthernetFrame Inject(SimulationEngine e,string source,string destination,byte ttl,ushort id=7,ushort sequence=1)
    {
        var frame=new EthernetFrame(Mac("02:00:00:00:01:01"),Mac("02:00:00:00:02:01"),new Ipv4Packet(Ip(source),Ip(destination),ttl,new IcmpPacket(IcmpKind.EchoRequest,id,sequence)));
        e.InjectFrameForTesting(Endpoint.Of("r","a"),frame,0);return frame;
    }
    private static SimulationEngine LoopReturnFixture()
    {
        var e=new SimulationEngine();e.CreateHost("h",["eth0"]);
        e.ConfigureHostIpv4(Endpoint.Of("h","eth0"),Mac("02:00:00:00:01:01"),new(new(Ip("192.168.1.10"),24),Ip("192.168.1.1")),true);
        e.CreateRouter("r1",[new("eth0",Mac("02:00:00:00:02:01"),new(Ip("192.168.1.1"),24),true),new("eth1",Mac("02:00:00:00:02:02"),new(Ip("172.16.0.1"),30),true)]);
        e.CreateRouter("r2",[new("eth0",Mac("02:00:00:00:03:01"),new(Ip("172.16.0.2"),30),true),new("eth1",Mac("02:00:00:00:03:02"),new(Ip("203.0.113.1"),24),false)]);
        e.ConnectLink("l1",Endpoint.Of("h","eth0"),Endpoint.Of("r1","eth0"));e.ConnectLink("l2",Endpoint.Of("r1","eth1"),Endpoint.Of("r2","eth0"));
        e.AddRouterStaticRoute("r1",new(Ip("10.0.0.0"),24),Ip("172.16.0.2"),"eth1",10);
        e.AddRouterStaticRoute("r2",new(Ip("10.0.0.0"),24),Ip("172.16.0.1"),"eth0",10);
        e.AddRouterStaticRoute("r2",new(Ip("192.168.1.0"),24),Ip("172.16.0.1"),"eth0",10);
        e.InjectFrameForTesting(Endpoint.Of("r1","eth0"),new(Mac("02:00:00:00:01:01"),Mac("02:00:00:00:02:01"),new Ipv4Packet(Ip("192.168.1.10"),Ip("10.0.0.20"),4,new IcmpPacket(IcmpKind.EchoRequest,1,1))),0);return e;
    }
    [Fact]
    public void TimeExceededReturnsAcrossRouterWithIntactSummary()
    {
        var e=LoopReturnFixture();Assert.Equal(ExecutionOutcome.Quiescent,e.RunUntilQuiescent(1000).Outcome);
        var tx=e.Trace.Records.OfType<FrameTransmitted>().Where(t=>t.Frame.Payload is Ipv4Packet).ToArray();
        Assert.Equal(new byte[]{3,2,1},tx.Select(t=>(Ipv4Packet)t.Frame.Payload).Where(p=>p.Payload is IcmpPacket).Select(p=>p.Ttl));
        var errors=tx.Where(t=>((Ipv4Packet)t.Frame.Payload).Payload is IcmpErrorMessage).ToArray();Assert.Equal(2,errors.Length);
        Assert.Equal(Endpoint.Of("r2","eth0"),errors[0].From);Assert.Equal(Endpoint.Of("r1","eth0"),errors[1].From);
        Assert.Equal(new byte[]{64,63},errors.Select(t=>((Ipv4Packet)t.Frame.Payload).Ttl));
        foreach (var errorTx in errors)
        {
            var p=(Ipv4Packet)errorTx.Frame.Payload;Assert.Equal(Ip("172.16.0.2"),p.SourceIp);Assert.Equal(Ip("192.168.1.10"),p.DestinationIp);
            var error=Assert.IsType<TimeExceededMessage>(p.Payload);Assert.Equal(new OriginalDatagramSummary(Ip("192.168.1.10"),Ip("10.0.0.20"),1,1,1),error.Original);
        }
        var drop=Assert.Single(e.Trace.Records.OfType<PacketDropped>());Assert.Equal("r2",drop.Device);Assert.Equal(DropReason.TtlExpiredInTransit,drop.Reason);Assert.Equal(1,drop.Ipv4Packet!.Ttl);
        Assert.Equal(9900UL,e.CurrentTimeNs);
    }
    private static string[] ObservationProjection(SimulationEngine e,string label)
    {
        static string Address(Ipv4Address a)=>$"{a.A}.{a.B}.{a.C}.{a.D}";
        static string MacText(MacAddress m)=>$"{m.A:x2}:{m.B:x2}:{m.C:x2}:{m.D:x2}:{m.E:x2}:{m.F:x2}";
        static string Ep(Endpoint p)=>$"{p.Device}:{p.InterfaceName}";
        static string Packet(Ipv4Packet p)
        {
            var message=p.Payload switch {
                IcmpErrorMessage error=>$"error,{error.Type},{error.Code},{Address(error.Original.SourceIp)},{Address(error.Original.DestinationIp)},{error.Original.Protocol},{error.Original.EchoIdentifier?.ToString()??"-"},{error.Original.EchoSequence?.ToString()??"-"}",
                IcmpPacket c=>$"echo,{c.Kind},{c.Identifier},{c.Sequence}",_=>throw new InvalidOperationException("Unknown ICMP")};
            return $"ip,{Address(p.SourceIp)},{Address(p.DestinationIp)},{p.Ttl},{message}";
        }
        static string Frame(EthernetFrame f)=>$"eth,{MacText(f.SourceMac)},{MacText(f.DestinationMac)},"+(f.Payload switch {
            Ipv4Packet p=>Packet(p),ArpPacket a=>$"arp,{a.Operation},{Address(a.SenderIp)},{MacText(a.SenderMac)},{Address(a.TargetIp)},{MacText(a.TargetMac)}",_=>throw new InvalidOperationException("Unknown frame")});
        var trace=e.Trace;var rows=new List<string>{$"ICMP4|{label}|result|{e.CurrentTimeNs}|{trace.TotalEmittedEver}|{trace.DroppedByCapacity}"};
        foreach (var r in trace.Records)
        {
            var detail=r switch {
                FrameTransmitted t=>$"tx|{t.Link}|{Ep(t.From)}|{Ep(t.To)}|{t.ArrivesAtNs}|{Frame(t.Frame)}",
                FrameDelivered t=>$"rx|{t.Link}|{Ep(t.AtEndpoint)}|{Frame(t.Frame)}",
                ArpPacketQueued q=>$"queue|{q.Device}|{Address(q.TargetIp)}|{Packet(q.Packet)}",
                TableUpdated {Update:ArpLearned a} t=>$"arp|{t.Device}|{Address(a.Ip)}|{MacText(a.Mac)}|{a.ExpiresAtNs}",
                PacketDropped d=>$"drop|{d.Device}|{d.InterfaceName??"-"}|{d.Reason}|"+(d.EthernetFrame is { } f ? Frame(f):Packet(d.Ipv4Packet!)),
                _=>throw new InvalidOperationException("Unexpected non-error-path observation")};
            rows.Add($"ICMP4|{label}|{r.TraceSequence}|{r.AtNs}|{r.ObservationInterface??"-"}|{detail}");
        }return rows.ToArray();
    }
    private static SimulationEngine DifferentialCase(string label)
    {
        var e=label=="loop-return" ? LoopReturnFixture() : Fixture();
        switch (label)
        {
            case "no-route":case "egress-failure":
                e.SendPing("h",Ip("198.51.100.1"),7,2);
                if (label=="egress-failure"){e.AdvanceUntil(3200,100);e.SetInterfaceAdminState(Endpoint.Of("r","a"),false);}break;
            case "ttl-one":Inject(e,"192.168.1.10","10.0.0.20",1,65535,23);break;
            case "ttl-zero":Inject(e,"192.168.1.10","203.0.113.99",0,8,4);break;
            case "no-return":Inject(e,"8.8.8.8","198.51.100.1",64,9,3);break;
            case "arp-failure":e.SetInterfaceAdminState(Endpoint.Of("h","eth0"),false);Inject(e,"192.168.1.10","198.51.100.1",1);break;
            case "loop-return":break;
            default:throw new ArgumentException("Unknown case",nameof(label));
        }
        Assert.Equal(ExecutionOutcome.Quiescent,e.RunUntilQuiescent(1000).Outcome);return e;
    }
    [Fact]
    public void IcmpCrossProjectionIsDeterministic()
    {
        foreach (var label in new[]{"no-route","ttl-one","ttl-zero","no-return","egress-failure","arp-failure","loop-return"})
        {
            var expected=ObservationProjection(DifferentialCase(label),label);
            for(var i=1;i<100;i++)Assert.Equal(expected,ObservationProjection(DifferentialCase(label),label));
            foreach(var row in expected)output.WriteLine(row);
        }
    }
    [Fact]
    public void OrdinaryRouterArpExhaustionKeepsPendingDiagnostic()
    {
        var e=LoopReturnFixture();e.SetInterfaceAdminState(Endpoint.Of("r2","eth0"),false);
        Assert.Equal(SimulationErrorCode.RouterIcmpErrorPathPending,Assert.Throws<SimulationException>(()=>e.RunUntilQuiescent(1000)).Code);
        Assert.DoesNotContain(e.Trace.Records.OfType<FrameTransmitted>(),t=>t.Frame.Payload is Ipv4Packet {Payload:IcmpErrorMessage});
        Assert.DoesNotContain(e.Trace.Records.OfType<PacketDropped>(),d=>d.Reason==DropReason.ArpResolutionFailed);
    }
    [Theory]
    [InlineData(1,7,1,"198.51.100.1")]
    [InlineData(0,17,91,"10.9.8.7")]
    public void TransitTtlOneGeneratesTimeExceeded(byte ttl,ushort id,ushort sequence,string destination)
    {
        var e=Fixture();var original=Inject(e,"192.168.1.10",destination,ttl,id,sequence);
        Assert.Equal(ExecutionOutcome.Quiescent,e.RunUntilQuiescent(1000).Outcome);
        var tx=Assert.Single(e.Trace.Records.OfType<FrameTransmitted>(),t=>t.Frame.Payload is Ipv4Packet);
        Assert.Equal(2300UL,tx.AtNs);var packet=Assert.IsType<Ipv4Packet>(tx.Frame.Payload);
        Assert.Equal(Ip("192.168.1.1"),packet.SourceIp);Assert.Equal(Ip("192.168.1.10"),packet.DestinationIp);Assert.Equal(64,packet.Ttl);
        var error=Assert.IsType<TimeExceededMessage>(packet.Payload);Assert.Equal((byte)11,error.Type);Assert.Equal((byte)0,error.Code);
        Assert.Equal(new OriginalDatagramSummary(Ip("192.168.1.10"),Ip(destination),1,id,sequence),error.Original);
        Assert.Equal(original,Assert.IsType<FrameDelivered>(e.Trace.Records[0]).Frame);
        var drop=Assert.Single(e.Trace.Records.OfType<PacketDropped>());Assert.Equal(0UL,drop.AtNs);Assert.Equal(DropReason.TtlExpiredInTransit,drop.Reason);Assert.Equal(ttl,drop.Ipv4Packet!.Ttl);
        Assert.Equal(3300UL,e.CurrentTimeNs);
    }
}
