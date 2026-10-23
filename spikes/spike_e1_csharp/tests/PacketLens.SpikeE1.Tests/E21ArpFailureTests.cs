using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class E21ArpFailureTests(Xunit.Abstractions.ITestOutputHelper output)
{
    private static Ipv4Address Ip(string value) => Ipv4Address.Parse(value);
    private static MacAddress Mac(int last) => MacAddress.Parse($"02:00:00:00:00:{last:x2}");
    private static Endpoint Ep(string device, string port) => Endpoint.Of(device, port);
    private static SimulationEngine Fixture()
    {
        var e = new SimulationEngine();
        e.CreateHost("h", ["p"]); e.ConfigureHostIpv4(Ep("h", "p"), Mac(1), new(new(Ip("192.168.1.10"), 24), Ip("192.168.1.1")), true);
        e.CreateHost("peer", ["p"]); e.ConfigureHostIpv4(Ep("peer", "p"), Mac(4), new(new(Ip("10.0.0.30"), 24)), true);
        e.CreateRouter("r", [new("a", Mac(2), new(Ip("192.168.1.1"), 24), true), new("b", Mac(3), new(Ip("10.0.0.1"), 24), true)]);
        e.ConnectLink("l1", Ep("h", "p"), Ep("r", "a")); e.ConnectLink("l2", Ep("r", "b"), Ep("peer", "p")); return e;
    }
    private static Ipv4Packet Request(ushort id) => new(Ip("192.168.1.10"), Ip("10.0.0.20"), 64, new IcmpPacket(IcmpKind.EchoRequest, id, 17));
    private static void Inject(SimulationEngine e, Ipv4Packet packet, ulong at = 0, Endpoint? endpoint = null)
    {
        var ep = endpoint ?? Ep("r", "a");
        e.InjectFrameForTesting(ep, new(Mac(1), ep.Device == "h" ? Mac(1) : Mac(2), packet), at);
    }
    private static FrameTransmitted[] Errors(SimulationEngine e) => e.Trace.Records.OfType<FrameTransmitted>().Where(t => t.Frame.Payload is Ipv4Packet { Payload: IcmpErrorMessage }).ToArray();
    private static void AssertIdle(SimulationEngine e)
    {
        Assert.Equal((0, 0), e.RouterArpWorkForTesting("r"));
        Assert.Equal(0, e.ActivePingCountForTesting("h")); Assert.Equal(0, e.ActivePingCountForTesting("peer")); Assert.Equal(0, e.ActivePingCountForTesting("r"));
        e.AddRouterStaticRoute("r", new(Ip("198.19.0.0"), 24), null, "a", 1);
    }
    [Fact]
    public void RouterArpTimeoutDrainsFifo()
    {
        var e = Fixture(); foreach (var id in new ushort[] { 3, 1, 2 }) Inject(e, Request(id));
        Assert.Equal(ExecutionOutcome.Quiescent, e.RunUntilQuiescent(1000).Outcome);
        Assert.Equal(new ulong[] { 100, 1_000_000_100, 2_000_000_100 }, e.Trace.Records.OfType<FrameTransmitted>().Where(t => t.From == Ep("r", "b") && t.Frame.Payload is ArpPacket { Operation: ArpOperation.Request } arp && arp.TargetIp == Ip("10.0.0.20")).Select(t => t.AtNs));
        var ds = e.Trace.Records.OfType<PacketDropped>().Where(d => d.Ipv4Packet is not null).ToArray(); Assert.Equal(3, ds.Length);
        Assert.Equal(new ushort[] { 3, 1, 2 }, ds.Select(d => d.Ipv4Packet!.Payload.Identifier));
        for (var i = 0; i < ds.Length; i++) { Assert.Equal(3_000_000_100UL, ds[i].AtNs); Assert.Equal("b", ds[i].InterfaceName); Assert.Equal(DropReason.ArpResolutionFailed, ds[i].Reason); Assert.Equal(Request(new ushort[] { 3, 1, 2 }[i]) with { Ttl = 63 }, ds[i].Ipv4Packet); }
        Assert.Equal(3, Errors(e).Length); AssertIdle(e);
    }
    [Fact]
    public void RouterArpTimeoutGeneratesEligibleHostUnreachable()
    {
        foreach (var id in new ushort[] { 7, 65535 })
        {
            var e = Fixture(); var original = Request(id); Inject(e, original); e.RunUntilQuiescent(1000);
            var tx = Assert.Single(Errors(e)); Assert.Equal(3_000_002_400UL, tx.AtNs); Assert.Equal(Ep("r", "a"), tx.From);
            var packet = Assert.IsType<Ipv4Packet>(tx.Frame.Payload); Assert.Equal(Ip("192.168.1.1"), packet.SourceIp); Assert.Equal(original.SourceIp, packet.DestinationIp); Assert.Equal(64, packet.Ttl);
            var error = Assert.IsType<DestinationHostUnreachableMessage>(packet.Payload); Assert.Equal((byte)3, error.Type); Assert.Equal((byte)1, error.Code); Assert.Equal(new OriginalDatagramSummary(Ip("192.168.1.10"), Ip("10.0.0.20"), 1, id, 17), error.Original);
            Assert.Equal(3_000_003_400UL, e.CurrentTimeNs); AssertIdle(e);
        }
    }
    [Fact]
    public void MixedArpQueueNeverGeneratesErrorOnError()
    {
        var e = Fixture(); var packets = new[] { Request(3), Request(99) with { Payload = new TimeExceededMessage(OriginalDatagramSummary.From(Request(3))) }, Request(2) };
        foreach (var p in packets) Inject(e, p); e.RunUntilQuiescent(1000);
        var drops = e.Trace.Records.OfType<PacketDropped>().Where(d => d.Ipv4Packet is not null).ToArray(); Assert.Equal(3, drops.Length);
        for (var i = 0; i < 3; i++) { Assert.Equal(packets[i] with { Ttl = 63 }, drops[i].Ipv4Packet); Assert.Equal(DropReason.ArpResolutionFailed, drops[i].Reason); }
        Assert.Equal(new ushort?[] { 3, 2 }, Errors(e).Select(t => Assert.IsType<DestinationHostUnreachableMessage>(Assert.IsType<Ipv4Packet>(t.Frame.Payload).Payload).Original.EchoIdentifier)); AssertIdle(e);
    }
    [Fact]
    public void ErrorArpFailureDoesNotRecurse()
    {
        var e = Fixture(); var p = Request(1) with { SourceIp = Ip("192.168.1.99") }; Inject(e, p); e.RunUntilQuiescent(1000);
        var drops = e.Trace.Records.OfType<PacketDropped>().Where(d => d.Ipv4Packet is not null).ToArray(); Assert.Equal(2, drops.Length);
        Assert.Equal(new ulong[] { 3_000_000_100, 6_000_000_200 }, drops.Select(d => d.AtNs)); Assert.All(drops, d => Assert.Equal(DropReason.ArpResolutionFailed, d.Reason));
        Assert.Equal(new OriginalDatagramSummary(Ip("192.168.1.99"), Ip("10.0.0.20"), 1, 1, 17), Assert.IsType<DestinationHostUnreachableMessage>(drops[1].Ipv4Packet!.Payload).Original); Assert.Empty(Errors(e)); AssertIdle(e);
    }
    [Fact]
    public void ReplyWithoutReturnRouteHasNormativeDrop()
    {
        foreach (var device in new[] { "h", "r" })
        {
            var e = Fixture(); var endpoint = Ep(device, device == "h" ? "p" : "a");
            if (device == "h") e.ConfigureHostIpv4(endpoint, Mac(1), new(new(Ip("192.168.1.10"), 24)), true);
            var request = Request(7) with { SourceIp = Ip("198.18.0.10"), DestinationIp = Ip(device == "h" ? "192.168.1.10" : "192.168.1.1"), Ttl = 1 }; Inject(e, request, endpoint: endpoint); e.RunUntilQuiescent(1000);
            var drop = Assert.Single(e.Trace.Records.OfType<PacketDropped>()); Assert.Equal(DropReason.NoRouteToHost, drop.Reason); Assert.Equal(0UL, drop.AtNs);
            Assert.Equal(new Ipv4Packet(request.DestinationIp, request.SourceIp, 64, new IcmpPacket(IcmpKind.EchoReply, 7, 17)), drop.Ipv4Packet); Assert.Empty(Errors(e)); Assert.Empty(e.Trace.Records.OfType<PingFailed>()); Assert.Empty(e.Trace.Records.OfType<PingCompleted>()); AssertIdle(e);
        }
    }
    [Fact]
    public void FailedReplyCreatesNoReceiverPing()
    {
        foreach (var gateway in new[] { (Ipv4Address?)null, Ip("10.0.0.99") })
        {
            var e = Fixture(); e.ConfigureHostIpv4(Ep("peer", "p"), Mac(4), new(new(Ip("10.0.0.20"), 24), gateway), true); e.SendPing("h", Ip("10.0.0.20"), 5, 9); e.RunUntilQuiescent(1000);
            var d = Assert.Single(e.Trace.Records.OfType<PacketDropped>(), d => d.Ipv4Packet is not null); Assert.Equal(gateway is null ? 6500UL : 3_000_006_600UL, d.AtNs); Assert.Equal("peer", d.Device);
            Assert.Equal(gateway is null ? DropReason.NoRouteToHost : DropReason.ArpResolutionFailed, d.Reason);
            Assert.Equal(new Ipv4Packet(Ip("10.0.0.20"), Ip("192.168.1.10"), 64, new IcmpPacket(IcmpKind.EchoReply, 5, 9)), d.Ipv4Packet);
            Assert.Empty(e.Trace.Records.OfType<PingFailed>()); Assert.Empty(e.Trace.Records.OfType<PingCompleted>());
            Assert.Equal(0, e.ActivePingCountForTesting("peer")); Assert.Equal(1, e.ActivePingCountForTesting("h"));
        }
    }
    [Fact]
    public void S3NoGatewayIsDropAndOneTerminalWithoutHiddenSession()
    {
        var e = Fixture(); e.ConfigureHostIpv4(Ep("h", "p"), Mac(1), new(new(Ip("192.168.1.10"), 24)), true);
        for (var i = 0; i < 2; i++) { e.SendPing("h", Ip("10.0.0.20"), 4, 1); Assert.Equal(ExecutionOutcome.Quiescent, e.RunUntilQuiescent(100).Outcome); }
        Assert.Equal(4, e.Trace.Records.Count); Assert.Equal(0UL, e.CurrentTimeNs);
        for (var i = 0; i < 4; i += 2)
        {
            var drop = Assert.IsType<PacketDropped>(e.Trace.Records[i]); Assert.Equal(DropReason.NoRouteToHost, drop.Reason); Assert.Equal(Ip("10.0.0.20"), drop.Ipv4Packet!.DestinationIp); Assert.Equal(64, drop.Ipv4Packet.Ttl);
            var failed = Assert.IsType<PingFailed>(e.Trace.Records[i + 1]); Assert.Equal("h", failed.Host); Assert.Equal(Ip("10.0.0.20"), failed.TargetIp); Assert.Equal((ushort)4, failed.Identifier); Assert.Equal((ushort)1, failed.Sequence); Assert.Equal(PingFailureReason.DestinationNetworkUnreachable, failed.Reason);
        }
        AssertIdle(e);
    }

    private static readonly string[] Cases = ["fifo", "transit-reply", "mixed-te", "mixed-net", "mixed-host", "no-return", "same-key", "return-arp-fail", "static-next", "return-static", "return-down", "egress-down", "egress-reconnect", "s3", "s7-no-gateway", "s7-bad-gateway", "host-mixed", "renew-expire", "selected-egress-down", "reply-host-exhaust", "reply-router-exhaust", "reply-host-down", "reply-router-down", "reply-host-link", "reply-router-link", "reply-host-reconnect", "reply-router-reconnect", "reply-host-flight", "reply-router-flight"];

    private static SimulationEngine RunCase(string label)
    {
        var e = Fixture(); var ingress = Ep("r", "a"); var exit = Ep("r", "b");
        if (label.StartsWith("s7-", StringComparison.Ordinal))
        {
            var gateway = label == "s7-no-gateway" ? (Ipv4Address?)null : Ip("10.0.0.99"); e.ConfigureHostIpv4(Ep("peer", "p"), Mac(4), new(new(Ip("10.0.0.20"), 24), gateway), true);
            e.SendPing("h", Ip("10.0.0.20"), 5, 9); Assert.Equal(ExecutionOutcome.Quiescent, e.RunUntilQuiescent(1000).Outcome);
            var drop = Assert.Single(e.Trace.Records.OfType<PacketDropped>(), d => d.Ipv4Packet is not null); Assert.Equal(gateway is null ? 6500UL : 3_000_006_600UL, drop.AtNs);
            Assert.Equal(gateway is null ? DropReason.NoRouteToHost : DropReason.ArpResolutionFailed, drop.Reason); Assert.Equal(new Ipv4Packet(Ip("10.0.0.20"), Ip("192.168.1.10"), 64, new IcmpPacket(IcmpKind.EchoReply, 5, 9)), drop.Ipv4Packet);
            Assert.Empty(Errors(e)); Assert.Empty(e.Trace.Records.OfType<PingFailed>()); Assert.Empty(e.Trace.Records.OfType<PingCompleted>()); Assert.Equal(1, e.ActivePingCountForTesting("h")); Assert.Equal(0, e.ActivePingCountForTesting("peer")); return e;
        }
        if (label.StartsWith("reply-", StringComparison.Ordinal))
        {
            var isHost = label.Contains("host", StringComparison.Ordinal); var receiver = isHost ? Ep("h", "p") : ingress;
            var source = Ip(label.Contains("exhaust", StringComparison.Ordinal) ? "192.168.1.99" : isHost ? "192.168.1.1" : "192.168.1.10"); var own = Ip(isHost ? "192.168.1.10" : "192.168.1.1");
            foreach (var id in new ushort[] { 3, 1, 2 }) Inject(e, Request(id) with { SourceIp = source, DestinationIp = own, Ttl = 1 }, endpoint: receiver);
            if (!label.Contains("exhaust", StringComparison.Ordinal))
            {
                e.AdvanceUntil(label.Contains("flight", StringComparison.Ordinal) ? 2300UL : 2200UL, 1000);
                if (label.Contains("down", StringComparison.Ordinal)) e.SetInterfaceAdminState(receiver, false);
                else { e.DisconnectLink("l1"); if (label.Contains("reconnect", StringComparison.Ordinal)) e.ConnectLink("l-new", Ep("h", "p"), ingress); }
            }
            Assert.Equal(ExecutionOutcome.Quiescent, e.RunUntilQuiescent(1000).Outcome); Assert.Empty(Errors(e)); Assert.Equal(0, e.ActivePingCountForTesting("h")); Assert.Equal(0, e.ActivePingCountForTesting("r"));
            var dropped = e.Trace.Records.OfType<PacketDropped>().Select(d => (Drop: d, Packet: d.Ipv4Packet ?? d.EthernetFrame?.Payload as Ipv4Packet)).Where(d => d.Packet is not null).ToArray();
            if (label.Contains("reconnect", StringComparison.Ordinal))
            {
                Assert.Empty(dropped); Assert.Equal(3300UL, e.CurrentTimeNs); Assert.Equal(3, e.Trace.Records.OfType<FrameTransmitted>().Count(t => t.Frame.Payload is Ipv4Packet));
            }
            else
            {
                Assert.Equal(3, dropped.Length);
                var reason = label.Contains("exhaust", StringComparison.Ordinal) ? DropReason.ArpResolutionFailed : label.Contains("down", StringComparison.Ordinal) ? DropReason.EgressInterfaceDown : label.Contains("flight", StringComparison.Ordinal) ? DropReason.LinkDisconnectedInFlight : DropReason.EgressLinkUnavailable;
                var at = label.Contains("exhaust", StringComparison.Ordinal) ? 3_000_000_100UL : label.Contains("flight", StringComparison.Ordinal) ? 3300UL : 2300UL;
                for (var i = 0; i < 3; i++) { Assert.Equal(at, dropped[i].Drop.AtNs); Assert.Equal(reason, dropped[i].Drop.Reason); Assert.Equal(new Ipv4Packet(own, source, 64, new IcmpPacket(IcmpKind.EchoReply, new ushort[] { 3, 1, 2 }[i], 17)), dropped[i].Packet); }
            }
            var total = e.Trace.TotalEmittedEver; e.AdvanceUntil(4_000_000_000, 1000); Assert.Equal(total, e.Trace.TotalEmittedEver); AssertIdle(e); return e;
        }
        switch (label)
        {
            case "s3":
                e = new SimulationEngine(); e.CreateHost("host1", ["eth0"]); var settings = new HostIpv4Settings(new(Ip("192.168.1.10"), 24));
                var mac = MacAddress.Parse("02:00:00:00:01:01"); e.ConfigureHostIpv4(Ep("host1", "eth0"), mac, settings, true); e.SendPing("host1", Ip("172.16.5.99"), 1, 1);
                Assert.Equal(ExecutionOutcome.Quiescent, e.RunUntilQuiescent(100).Outcome); Assert.Equal(0UL, e.CurrentTimeNs); Assert.Equal(2, e.Trace.Records.Count);
                Assert.Equal(DropReason.NoRouteToHost, Assert.IsType<PacketDropped>(e.Trace.Records[0]).Reason); var terminal = Assert.IsType<PingFailed>(e.Trace.Records[1]); Assert.Equal("host1", terminal.Host); Assert.Equal(Ip("172.16.5.99"), terminal.TargetIp); Assert.Equal((ushort)1, terminal.Identifier); Assert.Equal((ushort)1, terminal.Sequence); Assert.Equal(PingFailureReason.DestinationNetworkUnreachable, terminal.Reason);
                Assert.Equal(0, e.ActivePingCountForTesting("host1")); e.ConfigureHostIpv4(Ep("host1", "eth0"), mac, settings, true); return e;
            case "host-mixed":
                e.SendPing("h", Ip("192.168.1.99"), 3, 17); Inject(e, Request(1) with { SourceIp = Ip("192.168.1.99"), DestinationIp = Ip("192.168.1.10") }, endpoint: Ep("h", "p")); e.RunUntilQuiescent(1000);
                var mixedDrops = e.Trace.Records.OfType<PacketDropped>().Where(d => d.Ipv4Packet is not null).ToArray(); Assert.Equal(new ushort[] { 3, 1 }, mixedDrops.Select(d => d.Ipv4Packet!.Payload.Identifier));
                var failure = Assert.Single(e.Trace.Records.OfType<PingFailed>()); Assert.Equal("h", failure.Host); Assert.Equal((ushort)3, failure.Identifier); Assert.Equal(Ip("192.168.1.99"), failure.TargetIp); Assert.Equal(PingFailureReason.ArpTimeout, failure.Reason); Assert.Empty(Errors(e)); AssertIdle(e); return e;
        }
        if (label == "renew-expire")
        {
            var packet = Request(8) with { DestinationIp = Ip("10.0.0.30") }; Inject(e, packet); e.RunUntilQuiescent(1000); Assert.Equal(Mac(4), e.RouterArpEntry(exit, packet.DestinationIp));
            var before = e.Trace.TotalEmittedEver; e.AdvanceUntil(1_000_000_100, 1000); Assert.Equal(before, e.Trace.TotalEmittedEver);
            e.AdvanceUntil(60_000_002_200, 1000); Assert.Null(e.RouterArpEntry(exit, packet.DestinationIp));
            packet = packet with { Payload = new IcmpPacket(IcmpKind.EchoRequest, 9, 23) }; Inject(e, packet, e.CurrentTimeNs); e.RunUntilQuiescent(1000);
            Assert.Equal(Mac(4), e.RouterArpEntry(exit, packet.DestinationIp)); Assert.Equal(2, e.Trace.Records.OfType<ArpPacketQueued>().Count(q => q.Device == "r" && q.TargetIp == packet.DestinationIp));
            Assert.DoesNotContain(e.Trace.Records.OfType<PacketDropped>(), d => d.Reason == DropReason.ArpResolutionFailed); AssertIdle(e); return e;
        }
        if (label == "selected-egress-down")
        {
            var packet = Request(13) with { DestinationIp = Ip("198.51.100.7") }; e.AddRouterStaticRoute("r", new(packet.DestinationIp, 24), Ip("10.0.0.30"), "b", 1); e.AddRouterStaticRoute("r", new(Ip("0.0.0.0"), 0), Ip("192.168.1.10"), "a", 99);
            Inject(e, packet); e.AdvanceUntil(2200, 1000); e.SetInterfaceAdminState(exit, false); e.RunUntilQuiescent(1000); Assert.Empty(Errors(e));
            var drop = Assert.Single(e.Trace.Records.OfType<PacketDropped>(), d => d.EthernetFrame?.Payload is Ipv4Packet); Assert.Equal(DropReason.EgressInterfaceDown, drop.Reason); Assert.Equal(packet with { Ttl = 63 }, drop.EthernetFrame!.Payload); Assert.Equal(2300UL, drop.AtNs);
            Assert.DoesNotContain(e.Trace.Records.OfType<FrameTransmitted>(), t => t.Frame.Payload is Ipv4Packet); AssertIdle(e); return e;
        }
        var requests = (label == "fifo" || label.StartsWith("mixed-", StringComparison.Ordinal)) ? new[] { Request(3), Request(1), Request(2) } : [Request(3)];
        if (label == "transit-reply") requests[0] = requests[0] with { Payload = new IcmpPacket(IcmpKind.EchoReply, 3, 17) };
        if (label.StartsWith("mixed-", StringComparison.Ordinal))
        {
            var summary = OriginalDatagramSummary.From(requests[0]); IcmpMessage error = label switch { "mixed-te" => new TimeExceededMessage(summary), "mixed-net" => new DestinationNetworkUnreachableMessage(summary), _ => new DestinationHostUnreachableMessage(summary) };
            requests[1] = requests[1] with { Payload = error };
        }
        if (label == "no-return") requests[0] = requests[0] with { SourceIp = Ip("198.18.0.10") };
        if (label == "same-key") requests[0] = requests[0] with { SourceIp = Ip("10.0.0.20") };
        if (label == "return-arp-fail") requests[0] = requests[0] with { SourceIp = Ip("192.168.1.99") };
        if (label == "static-next") { requests[0] = requests[0] with { DestinationIp = Ip("198.51.100.7") }; e.AddRouterStaticRoute("r", new(Ip("198.51.100.7"), 24), Ip("10.0.0.99"), "b", 9); }
        if (label == "return-static")
        {
            e.DisconnectLink("l2"); e.CreateRouter("returner", [new("a", Mac(5), new(Ip("10.0.0.30"), 24), true), new("b", Mac(6), new(Ip("203.0.113.1"), 24), false)]);
            e.ConnectLink("l-return", exit, Ep("returner", "a")); e.AddRouterStaticRoute("r", new(Ip("192.168.1.10"), 32), Ip("10.0.0.30"), "b", 99);
        }
        foreach (var request in requests) Inject(e, request);
        if (label is "return-down" or "egress-down" or "egress-reconnect")
        {
            e.AdvanceUntil(100, 1000);
            if (label == "return-down") e.SetInterfaceAdminState(ingress, false);
            else if (label == "egress-down") e.SetInterfaceAdminState(exit, false);
            else { e.DisconnectLink("l2"); e.ConnectLink("l-new", exit, Ep("peer", "p")); }
        }
        Assert.Equal(ExecutionOutcome.Quiescent, e.RunUntilQuiescent(1000).Outcome);
        var drops = e.Trace.Records.OfType<PacketDropped>().Where(d => d.Ipv4Packet is not null).ToArray(); var eligible = requests.Count(p => p.Payload is not IcmpErrorMessage);
        Assert.Equal(eligible, drops.Count(d => d.Ipv4Packet!.Payload is not IcmpErrorMessage));
        foreach (var request in requests) Assert.Contains(e.Trace.Records.OfType<FrameDelivered>(), r => Equals(r.Frame.Payload, request));
        foreach (var drop in drops.Where(d => d.AtNs == 3_000_000_100)) { Assert.Equal(63, drop.Ipv4Packet!.Ttl); Assert.Equal(DropReason.ArpResolutionFailed, drop.Reason); }
        var emitted = Errors(e);
        if (label is "no-return" or "return-down" or "same-key" or "return-arp-fail")
        {
            Assert.Empty(emitted); Assert.Equal(label is "same-key" or "return-arp-fail" ? 2 : 1, drops.Length);
            if (label is "same-key" or "return-arp-fail") { Assert.Equal(6_000_000_200UL, drops[1].AtNs); Assert.Equal(DropReason.ArpResolutionFailed, drops[1].Reason); Assert.IsType<DestinationHostUnreachableMessage>(drops[1].Ipv4Packet!.Payload); }
        }
        else
        {
            Assert.Equal(eligible, emitted.Length);
            foreach (var tx in emitted)
            {
                Assert.Equal(label == "return-static" ? exit : ingress, tx.From); var p = Assert.IsType<Ipv4Packet>(tx.Frame.Payload);
                Assert.Equal(Ip(label == "return-static" ? "10.0.0.1" : "192.168.1.1"), p.SourceIp); Assert.Equal(64, p.Ttl);
                var error = Assert.IsType<DestinationHostUnreachableMessage>(p.Payload); Assert.Equal((byte)3, error.Type); Assert.Equal((byte)1, error.Code);
            }
        }
        Assert.Empty(e.Trace.Records.OfType<PingFailed>()); Assert.Empty(e.Trace.Records.OfType<PingCompleted>());
        AssertIdle(e); return e;
    }
    [Fact] public void FailureVariationsHaveNormativeOracles() { foreach (var label in Cases) RunCase(label); }
    private static string[] Projection(SimulationEngine e, string label)
    {
        Assert.All(e.Trace.Records.Zip(e.Trace.Records.Skip(1)), pair => { Assert.True(pair.First.AtNs <= pair.Second.AtNs); Assert.True(pair.First.TraceSequence < pair.Second.TraceSequence); });
        var rows = E21RouterIcmpTests.ObservationProjection(e, label).Select(row => row.Replace("ICMP4|", "ARP5|", StringComparison.Ordinal)).ToList();
        if (label.StartsWith("s7-", StringComparison.Ordinal)) rows.Add($"ARP5|{label}|sessions|h:{e.ActivePingCountForTesting("h")}|peer:{e.ActivePingCountForTesting("peer")}");
        foreach (var endpoint in label == "s3" ? Array.Empty<Endpoint>() : new[] { Ep("r", "a"), Ep("r", "b") }) foreach (var ip in new[] { Ip("192.168.1.10"), Ip("192.168.1.1"), Ip("10.0.0.20"), Ip("10.0.0.30"), Ip("192.168.1.99") })
        {
            var cached = e.RouterArpEntry(endpoint, ip); var value = cached is { } m ? $"Some([{m.A}, {m.B}, {m.C}, {m.D}, {m.E}, {m.F}])" : "None";
            rows.Add($"ARP5|{label}|state|{endpoint.Device}:{endpoint.InterfaceName}|[{ip.A}, {ip.B}, {ip.C}, {ip.D}]|{value}");
        }
        return rows.ToArray();
    }
    [Fact]
    public void ArpFailureProjectionIsDeterministic()
    {
        foreach (var label in Cases) { var expected = Projection(RunCase(label), label); for (var i = 1; i < 100; i++) Assert.Equal(expected, Projection(RunCase(label), label)); foreach (var row in expected) output.WriteLine(row); }
    }
}
