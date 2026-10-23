namespace PacketLens.SpikeE1.Domain;

// Semantic data only: not bytes quoted from a wire header.
public sealed record OriginalDatagramSummary(Ipv4Address SourceIp, Ipv4Address DestinationIp,
    byte Protocol, ushort? EchoIdentifier, ushort? EchoSequence)
{
    public static OriginalDatagramSummary From(Ipv4Packet packet)
    {
        var echo = packet.Payload as IcmpPacket;
        var isEcho = echo?.Kind is IcmpKind.EchoRequest or IcmpKind.EchoReply;
        return new(packet.SourceIp, packet.DestinationIp, 1, isEcho ? echo!.Identifier : null, isEcho ? echo!.Sequence : null);
    }
}
public abstract record IcmpErrorMessage(OriginalDatagramSummary Original) : IcmpMessage
{
    public abstract byte Type { get; }
    public byte Code => 0;
}
public sealed record TimeExceededMessage(OriginalDatagramSummary Original) : IcmpErrorMessage(Original)
{
    public override IcmpKind Kind => IcmpKind.TimeExceeded;
    public override byte Type => 11;
}
public sealed record DestinationNetworkUnreachableMessage(OriginalDatagramSummary Original) : IcmpErrorMessage(Original)
{
    public override IcmpKind Kind => IcmpKind.DestinationNetworkUnreachable;
    public override byte Type => 3;
}
