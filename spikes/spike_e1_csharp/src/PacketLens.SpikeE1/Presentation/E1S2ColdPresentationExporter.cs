using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;

namespace PacketLens.SpikeE1.Presentation;

/// <summary>
/// Read adapter for the cold, normative portion of E1-S2. It creates no
/// protocol behavior; it only configures the already specified scenario and
/// projects its observable trace to presentation/v1.
/// </summary>
public static class E1S2ColdPresentationExporter
{
    public static SimulationPresentationSnapshot Export()
    {
        var engine = CreateE1S2Engine();
        engine.SendPing("pc1", Ipv4Address.Parse("192.168.1.20"), 1, 1);
        var execution = engine.RunUntilQuiescent(100);
        if (execution.Outcome != ExecutionOutcome.Quiescent)
        {
            throw new InvalidOperationException($"E1-S2 cold did not quiesce: {execution.Outcome}.");
        }

        var trace = engine.Trace;
        var snapshot = new SimulationPresentationSnapshot(
            SimulationPresentationSnapshot.CurrentSchemaVersion,
            "E1-S2",
            "cold",
            CreateTopology(),
            new PresentationExecution(execution.CurrentTimeNs.ToString(), ExecutionOutcomeCode(execution.Outcome)),
            new TraceRetention(
                trace.TotalEmittedEver.ToString(),
                trace.DroppedByCapacity.ToString(),
                trace.OldestAvailableSequence?.ToString(),
                trace.NewestAvailableSequence?.ToString()),
            trace.Records.Select(Project).ToArray());

        var errors = snapshot.Validate();
        if (errors.Count != 0)
        {
            throw new InvalidOperationException($"E1-S2 cold projection violates the contract: {errors[0].Code}.");
        }
        return snapshot;
    }

    private static SimulationEngine CreateE1S2Engine()
    {
        var engine = new SimulationEngine();
        engine.CreateHost("pc1", ["eth0"]);
        engine.CreateHost("pc2", ["eth0"]);
        engine.CreateHost("pc3", ["eth0"]);
        engine.CreateL2Switch("sw1", ["port1", "port2", "port3", "port4"]);
        ConfigureHost(engine, "pc1", "00:00:00:00:00:01", "192.168.1.10");
        ConfigureHost(engine, "pc2", "00:00:00:00:00:02", "192.168.1.20");
        ConfigureHost(engine, "pc3", "00:00:00:00:00:03", "192.168.1.30");
        foreach (var port in new[] { "port1", "port2", "port3", "port4" })
        {
            engine.ConfigureInterface(Endpoint.Of("sw1", port), MacAddress.Zero, null, 0, true);
        }
        engine.ConnectLink("link1", Endpoint.Of("pc1", "eth0"), Endpoint.Of("sw1", "port1"));
        engine.ConnectLink("link2", Endpoint.Of("pc2", "eth0"), Endpoint.Of("sw1", "port2"));
        engine.ConnectLink("link3", Endpoint.Of("pc3", "eth0"), Endpoint.Of("sw1", "port3"));
        return engine;
    }

    private static void ConfigureHost(SimulationEngine engine, string device, string mac, string ip) =>
        engine.ConfigureInterface(Endpoint.Of(device, "eth0"), MacAddress.Parse(mac), Ipv4Address.Parse(ip), 24, true);

    private static PresentationTopology CreateTopology() => new(
        [
            new PresentationDevice("pc1", "host"),
            new PresentationDevice("pc2", "host"),
            new PresentationDevice("pc3", "host"),
            new PresentationDevice("sw1", "l2_switch")
        ],
        [
            new PresentationInterface("pc1:eth0", "pc1", "eth0"),
            new PresentationInterface("pc2:eth0", "pc2", "eth0"),
            new PresentationInterface("pc3:eth0", "pc3", "eth0"),
            new PresentationInterface("sw1:port1", "sw1", "port1"),
            new PresentationInterface("sw1:port2", "sw1", "port2"),
            new PresentationInterface("sw1:port3", "sw1", "port3"),
            new PresentationInterface("sw1:port4", "sw1", "port4")
        ],
        [
            new PresentationLink("link1", "pc1:eth0", "sw1:port1"),
            new PresentationLink("link2", "pc2:eth0", "sw1:port2"),
            new PresentationLink("link3", "pc3:eth0", "sw1:port3")
        ]);

    private static PresentationEvent Project(TraceEvent trace)
    {
        var empty = new PresentationEvent(
            trace.TraceSequence.ToString(),
            trace.AtNs.ToString(),
            TraceKind(trace),
            new PresentationActor(string.Empty, null),
            null,
            null,
            null,
            null,
            Array.Empty<string>(),
            null,
            null,
            null,
            null);

        return trace switch
        {
            FrameTransmitted transmitted => empty with
            {
                Actor = Actor(transmitted.From),
                Egress = EndpointFrom(transmitted.From),
                Destination = EndpointFrom(transmitted.To),
                LinkId = transmitted.Link,
                Protocols = Protocols(transmitted.Frame),
                Pdu = new PresentationPduObservation("outbound", ToPdu(transmitted.Frame))
            },
            FrameDelivered delivered => empty with
            {
                Actor = Actor(delivered.AtEndpoint),
                Ingress = EndpointFrom(delivered.AtEndpoint),
                LinkId = delivered.Link,
                Protocols = Protocols(delivered.Frame),
                Pdu = new PresentationPduObservation("inbound", ToPdu(delivered.Frame))
            },
            PacketDropped dropped => empty with
            {
                Actor = new PresentationActor(dropped.Device, dropped.InterfaceName is null ? null : InterfaceId(dropped.Device, dropped.InterfaceName)),
                Protocols = Protocols(dropped),
                Pdu = new PresentationPduObservation("dropped", ToPdu(dropped)),
                DropReason = DropReasonCode(dropped.Reason)
            },
            ArpPacketQueued queued => empty with
            {
                Actor = new PresentationActor(queued.Device, null),
                Protocols = ["arp", "ipv4", "icmp"],
                Pdu = new PresentationPduObservation("queued", ToPdu(queued.Packet))
            },
            TableUpdated updated => empty with
            {
                Actor = new PresentationActor(updated.Device, null),
                Protocols = Protocols(updated.Update),
                TableDelta = ToDelta(updated.Device, updated.Update)
            },
            PingCompleted completed => empty with
            {
                Actor = new PresentationActor(completed.Host, null),
                Protocols = ["icmp"],
                Ping = new PresentationPingResult(Ip(completed.TargetIp), completed.Identifier.ToString(), completed.Sequence.ToString(), completed.RttNs.ToString(), null)
            },
            PingFailed failed => empty with
            {
                Actor = new PresentationActor(failed.Host, null),
                Protocols = ["icmp"],
                Ping = new PresentationPingResult(Ip(failed.TargetIp), failed.Identifier.ToString(), failed.Sequence.ToString(), null, failed.Reason switch { PingFailureReason.ArpTimeout => "arp_timeout", _ => throw new ArgumentOutOfRangeException(nameof(failed.Reason)) })
            },
            _ => throw new ArgumentOutOfRangeException(nameof(trace), trace.GetType().Name)
        };
    }

    private static PresentationActor Actor(Endpoint endpoint) => new(endpoint.Device, InterfaceId(endpoint.Device, endpoint.InterfaceName));
    private static PresentationEndpoint EndpointFrom(Endpoint endpoint) => new(endpoint.Device, InterfaceId(endpoint.Device, endpoint.InterfaceName));
    private static string InterfaceId(string device, string interfaceName) => $"{device}:{interfaceName}";

    private static PresentationPdu ToPdu(EthernetFrame frame) => new PresentationEthernetPdu(Mac(frame.SourceMac), Mac(frame.DestinationMac), frame.Payload switch
    {
        ArpPacket arp => new PresentationArpPayload(arp.Operation switch { ArpOperation.Request => "request", ArpOperation.Reply => "reply", _ => throw new ArgumentOutOfRangeException(nameof(arp.Operation)) }, Mac(arp.SenderMac), Ip(arp.SenderIp), Mac(arp.TargetMac), Ip(arp.TargetIp)),
        Ipv4Packet ipv4 => new PresentationIpv4EthernetPayload(Ip(ipv4.SourceIp), Ip(ipv4.DestinationIp), ipv4.Ttl.ToString(), ToIpv4Payload(ipv4)),
        _ => throw new ArgumentOutOfRangeException(nameof(frame.Payload))
    });

    private static PresentationPdu ToPdu(Ipv4Packet packet) => new PresentationIpv4Pdu(Ip(packet.SourceIp), Ip(packet.DestinationIp), packet.Ttl.ToString(), ToIpv4Payload(packet));

    private static PresentationPdu ToPdu(PacketDropped dropped) => dropped switch
    {
        { EthernetFrame: { } frame, Ipv4Packet: null } => ToPdu(frame),
        { EthernetFrame: null, Ipv4Packet: { } packet } => ToPdu(packet),
        _ => throw new InvalidOperationException("PacketDropped must contain exactly one semantic PDU.")
    };

    private static PresentationIpv4Payload ToIpv4Payload(Ipv4Packet packet) =>
        new PresentationIcmpPayload(packet.Payload.Kind switch
        {
            IcmpKind.EchoRequest => "echo_request",
            IcmpKind.EchoReply => "echo_reply",
            IcmpKind.DestinationHostUnreachable => "destination_host_unreachable",
            IcmpKind.TimeExceeded => "time_exceeded",
            _ => throw new ArgumentOutOfRangeException(nameof(packet.Payload.Kind))
        }, packet.Payload.Identifier.ToString(), packet.Payload.Sequence.ToString());

    private static PresentationTableDelta ToDelta(string device, TableUpdate update) => update switch
    {
        ArpLearned learned => new PresentationArpLearned(Ip(learned.Ip), Mac(learned.Mac), learned.ExpiresAtNs.ToString()),
        ArpEntryExpired expired => new PresentationArpEntryExpired(Ip(expired.Ip)),
        MacLearned learned => new PresentationMacLearned(Mac(learned.Mac), InterfaceId(device, learned.Port), learned.ExpiresAtNs.ToString()),
        MacEntryExpired expired => new PresentationMacEntryExpired(Mac(expired.Mac)),
        _ => throw new ArgumentOutOfRangeException(nameof(update), update.GetType().Name)
    };

    private static IReadOnlyList<string> Protocols(EthernetFrame frame) => frame.Payload switch
    {
        ArpPacket => ["arp"],
        Ipv4Packet => ["ipv4", "icmp"],
        _ => throw new ArgumentOutOfRangeException(nameof(frame.Payload))
    };

    private static IReadOnlyList<string> Protocols(PacketDropped dropped) => dropped switch
    {
        { EthernetFrame: { } frame } => Protocols(frame),
        { Ipv4Packet: not null } => ["ipv4", "icmp"],
        _ => throw new InvalidOperationException("PacketDropped has no semantic PDU.")
    };

    private static IReadOnlyList<string> Protocols(TableUpdate update) => update switch
    {
        ArpLearned or ArpEntryExpired => ["arp"],
        MacLearned or MacEntryExpired => ["ethernet"],
        _ => throw new ArgumentOutOfRangeException(nameof(update), update.GetType().Name)
    };

    private static string TraceKind(TraceEvent trace) => trace switch
    {
        FrameTransmitted => "frame_transmitted",
        FrameDelivered => "frame_delivered",
        PacketDropped => "packet_dropped",
        ArpPacketQueued => "arp_packet_queued",
        TableUpdated => "table_updated",
        PingCompleted => "ping_completed",
        PingFailed => "ping_failed",
        _ => throw new ArgumentOutOfRangeException(nameof(trace), trace.GetType().Name)
    };

    private static string ExecutionOutcomeCode(ExecutionOutcome outcome) => outcome switch
    {
        ExecutionOutcome.Idle => "Idle",
        ExecutionOutcome.Quiescent => "Quiescent",
        ExecutionOutcome.TargetTimeReached => "TargetTimeReached",
        ExecutionOutcome.BudgetExhausted => "BudgetExhausted",
        ExecutionOutcome.HaltedBySafetyGuard => "HaltedBySafetyGuard",
        _ => throw new ArgumentOutOfRangeException(nameof(outcome))
    };

    private static string DropReasonCode(Domain.DropReason reason) => reason switch
    {
        Domain.DropReason.LinkDisconnectedInFlight => "link_disconnected_in_flight",
        Domain.DropReason.IngressInterfaceDown => "ingress_interface_down",
        Domain.DropReason.EgressInterfaceDown => "egress_interface_down",
        Domain.DropReason.EgressLinkUnavailable => "egress_link_unavailable",
        Domain.DropReason.ArpResolutionFailed => "arp_resolution_failed",
        Domain.DropReason.ArpTargetIpMismatch => "arp_target_ip_mismatch",
        _ => throw new ArgumentOutOfRangeException(nameof(reason))
    };

    private static string Mac(MacAddress address) => $"{address.A:x2}:{address.B:x2}:{address.C:x2}:{address.D:x2}:{address.E:x2}:{address.F:x2}";
    private static string Ip(Ipv4Address address) => $"{address.A}.{address.B}.{address.C}.{address.D}";
}
