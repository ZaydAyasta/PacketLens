namespace PacketLens.SpikeE1.Domain;

public abstract record EthernetPayload;

public enum ArpOperation { Request, Reply }

public sealed record ArpPacket(ArpOperation Operation, MacAddress SenderMac, Ipv4Address SenderIp, MacAddress TargetMac, Ipv4Address TargetIp) : EthernetPayload;

public enum IcmpKind { EchoRequest, EchoReply, DestinationHostUnreachable, TimeExceeded, DestinationNetworkUnreachable }

public abstract record IcmpMessage
{
    public abstract IcmpKind Kind { get; }
    // Legacy Echo readers remain source-compatible; errors have no exterior Echo identity.
    public virtual ushort Identifier => throw new InvalidOperationException("ICMP errors have no Echo Identifier; inspect Original instead.");
    public virtual ushort Sequence => throw new InvalidOperationException("ICMP errors have no Echo Sequence; inspect Original instead.");
    protected virtual bool PrintMembers(System.Text.StringBuilder builder)
    {
        builder.Append("Kind = ").Append(Kind);
        return true;
    }
}
public sealed record IcmpPacket : IcmpMessage
{
    public IcmpPacket(IcmpKind kind, ushort identifier, ushort sequence) { Kind = kind; Identifier = identifier; Sequence = sequence; }
    public override IcmpKind Kind { get; }
    public override ushort Identifier { get; }
    public override ushort Sequence { get; }
}

public sealed record Ipv4Packet(Ipv4Address SourceIp, Ipv4Address DestinationIp, byte Ttl, IcmpMessage Payload) : EthernetPayload;

public sealed record EthernetFrame(MacAddress SourceMac, MacAddress DestinationMac, EthernetPayload Payload);
