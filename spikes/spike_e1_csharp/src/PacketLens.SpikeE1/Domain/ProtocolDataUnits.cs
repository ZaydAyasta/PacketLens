namespace PacketLens.SpikeE1.Domain;

public abstract record EthernetPayload;

public enum ArpOperation { Request, Reply }

public sealed record ArpPacket(ArpOperation Operation, MacAddress SenderMac, Ipv4Address SenderIp, MacAddress TargetMac, Ipv4Address TargetIp) : EthernetPayload;

public enum IcmpKind { EchoRequest, EchoReply, DestinationHostUnreachable, TimeExceeded }

public sealed record IcmpPacket(IcmpKind Kind, ushort Identifier, ushort Sequence);

public sealed record Ipv4Packet(Ipv4Address SourceIp, Ipv4Address DestinationIp, byte Ttl, IcmpPacket Payload) : EthernetPayload;

public sealed record EthernetFrame(MacAddress SourceMac, MacAddress DestinationMac, EthernetPayload Payload);
