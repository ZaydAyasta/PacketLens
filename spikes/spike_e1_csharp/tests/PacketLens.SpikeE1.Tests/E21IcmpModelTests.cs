using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class E21IcmpModelTests
{
    [Theory]
    [InlineData(7, 1, "198.51.100.1")]
    [InlineData(65535, 23, "203.0.113.77")]
    public void ErrorSummaryPreservesOriginal(ushort id, ushort sequence, string destination)
    {
        var original = new Ipv4Packet(Ipv4Address.Parse("192.168.1.10"), Ipv4Address.Parse(destination), 1,
            new IcmpPacket(IcmpKind.EchoRequest, id, sequence));
        var before = original with { };
        var summary = OriginalDatagramSummary.From(original);
        Assert.Equal(original.SourceIp, summary.SourceIp);
        Assert.Equal(original.DestinationIp, summary.DestinationIp);
        Assert.Equal((byte)1, summary.Protocol);
        Assert.Equal(id, summary.EchoIdentifier);
        Assert.Equal(sequence, summary.EchoSequence);
        var reply = original with { Payload = new IcmpPacket(IcmpKind.EchoReply,id,sequence) };
        Assert.Equal(summary, OriginalDatagramSummary.From(reply));
        foreach (var (error, expectedType) in new (IcmpErrorMessage, byte)[] { (new TimeExceededMessage(summary), 11), (new DestinationNetworkUnreachableMessage(summary), 3) })
        {
            Assert.Equal((byte)0, error.Code);
            Assert.Equal(summary, error.Original);
            Assert.Equal(expectedType, error.Type);
            Assert.Contains("Original", error.ToString());
            Assert.Throws<InvalidOperationException>(() => error.Identifier);
            Assert.Throws<InvalidOperationException>(() => error.Sequence);
            var outer = new Ipv4Packet(Ipv4Address.Parse("192.168.1.1"), original.SourceIp, 64, error);
            var nested = OriginalDatagramSummary.From(outer);
            Assert.Null(nested.EchoIdentifier);
            Assert.Null(nested.EchoSequence);
            Assert.Equal(before, original);
        }
    }
}
