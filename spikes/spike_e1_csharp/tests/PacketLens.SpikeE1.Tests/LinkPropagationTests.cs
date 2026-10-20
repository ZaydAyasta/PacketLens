using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class LinkPropagationTests
{
    [Fact]
    public void SendPing_ArpRequestCrossesDirectLinkAfterExactlyOneThousandNanoseconds()
    {
        var engine = new SimulationEngine();
        engine.CreateHost("pc1", ["eth0"]);
        engine.CreateHost("pc2", ["eth0"]);
        engine.ConfigureInterface(Endpoint.Of("pc1", "eth0"), MacAddress.Parse("00:00:00:00:00:01"), Ipv4Address.Parse("192.168.1.10"), 24, true);
        engine.ConfigureInterface(Endpoint.Of("pc2", "eth0"), MacAddress.Parse("00:00:00:00:00:02"), Ipv4Address.Parse("192.168.1.20"), 24, true);
        engine.ConnectLink("link1", Endpoint.Of("pc1", "eth0"), Endpoint.Of("pc2", "eth0"));

        engine.SendPing("pc1", Ipv4Address.Parse("192.168.1.20"), 1, 1);
        engine.AdvanceUntil(1_000, 10);

        var delivery = Assert.Single(engine.Trace.Records.OfType<FrameDelivered>());
        Assert.Equal(1_000UL, delivery.AtNs);
        Assert.Equal(Endpoint.Of("pc2", "eth0"), delivery.AtEndpoint);
        var arp = Assert.IsType<ArpPacket>(delivery.Frame.Payload);
        Assert.Equal(ArpOperation.Request, arp.Operation);
    }
}
