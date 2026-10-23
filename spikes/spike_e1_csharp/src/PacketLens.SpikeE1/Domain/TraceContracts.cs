namespace PacketLens.SpikeE1.Domain;

public abstract record TraceEvent(ulong AtNs)
{
    public ulong TraceSequence { get; internal set; }
    // Domain-only E2 scope; existing presentation/v1 mappings remain unchanged.
    public string? ObservationInterface { get; internal init; }
}

public sealed record FrameTransmitted(ulong AtNs, string Link, Endpoint From, Endpoint To, ulong ArrivesAtNs, EthernetFrame Frame) : TraceEvent(AtNs);

public sealed record FrameDelivered(ulong AtNs, string Link, Endpoint AtEndpoint, EthernetFrame Frame) : TraceEvent(AtNs);

public sealed record ArpPacketQueued(ulong AtNs, string Device, Ipv4Address TargetIp, Ipv4Packet Packet) : TraceEvent(AtNs);

public sealed record PingCompleted(ulong AtNs, string Host, Ipv4Address TargetIp, ushort Identifier, ushort Sequence, ulong RttNs) : TraceEvent(AtNs);

public enum DropReason
{
    LinkDisconnectedInFlight,
    IngressInterfaceDown,
    EgressInterfaceDown,
    EgressLinkUnavailable,
    ArpResolutionFailed,
    ArpTargetIpMismatch,
    DestinationMacMismatch,
    NoRouteToHost,
    TtlExpiredInTransit,
    UnsupportedIpDestination,
    InvalidIpv4Source
}

public enum PingFailureReason { ArpTimeout, DestinationNetworkUnreachable }

public sealed record PacketDropped(ulong AtNs, string Device, string? InterfaceName, EthernetFrame? EthernetFrame, Ipv4Packet? Ipv4Packet, DropReason Reason) : TraceEvent(AtNs);

public sealed record PingFailed(ulong AtNs, string Host, Ipv4Address TargetIp, ushort Identifier, ushort Sequence, PingFailureReason Reason) : TraceEvent(AtNs);

public sealed record TableUpdated(ulong AtNs, string Device, TableUpdate Update) : TraceEvent(AtNs);

public abstract record TableUpdate;

public sealed record ArpLearned(Ipv4Address Ip, MacAddress Mac, ulong ExpiresAtNs) : TableUpdate;

public sealed record ArpEntryExpired(Ipv4Address Ip) : TableUpdate;

public sealed record MacLearned(MacAddress Mac, string Port, ulong ExpiresAtNs) : TableUpdate;

public sealed record MacEntryExpired(MacAddress Mac) : TableUpdate;

public sealed record TraceSnapshot(ulong TotalEmittedEver, ulong DroppedByCapacity, ulong? OldestAvailableSequence, ulong? NewestAvailableSequence, IReadOnlyList<TraceEvent> Records);
