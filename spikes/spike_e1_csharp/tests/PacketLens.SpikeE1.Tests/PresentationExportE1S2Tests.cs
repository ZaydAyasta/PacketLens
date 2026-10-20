using PacketLens.SpikeE1.Presentation;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class PresentationExportE1S2Tests
{
    [Fact]
    public void E1S2Cold_ExportsTheObservableTraceIntoPresentationV1()
    {
        var snapshot = E1S2ColdPresentationExporter.Export();

        Assert.Equal("presentation/v1", snapshot.SchemaVersion);
        Assert.Equal("E1-S2", snapshot.ScenarioId);
        Assert.Equal("cold", snapshot.RunLabel);
        Assert.Equal("Quiescent", snapshot.Execution.Outcome);
        Assert.Equal("8700", snapshot.Execution.FinalVirtualTimeNs);
        Assert.Equal(27, snapshot.Events.Count);
        Assert.Equal("27", snapshot.TraceRetention.TotalEmittedEver);
        Assert.Equal("0", snapshot.TraceRetention.DroppedByCapacity);
        Assert.Equal("1", snapshot.TraceRetention.OldestAvailableTraceSeq);
        Assert.Equal("27", snapshot.TraceRetention.NewestAvailableTraceSeq);
        Assert.Empty(snapshot.Validate());

        Assert.Equal(Enumerable.Range(1, 27).Select(value => value.ToString()), snapshot.Events.Select(traceEvent => traceEvent.TraceSeq));
        var arp = snapshot.Events.First(traceEvent => traceEvent.Kind == "frame_transmitted" && traceEvent.Protocols.SequenceEqual(["arp"]));
        Assert.Equal("0", arp.AtNs);
        var ethernet = Assert.IsType<PresentationEthernetPdu>(Assert.IsType<PresentationPduObservation>(arp.Pdu).Pdu);
        var arpPayload = Assert.IsType<PresentationArpPayload>(ethernet.Payload);
        Assert.Equal("00:00:00:00:00:01", ethernet.SourceMac);
        Assert.Equal("ff:ff:ff:ff:ff:ff", ethernet.DestinationMac);
        Assert.Equal("request", arpPayload.Operation);
        Assert.Equal("192.168.1.20", arpPayload.TargetIp);
        Assert.Contains(snapshot.Events, traceEvent => traceEvent.TableDelta is PresentationArpLearned { Ip: "192.168.1.20", Mac: "00:00:00:00:00:02", ExpiresAtNs: "60000004300" });
        Assert.Contains(snapshot.Events, traceEvent => traceEvent.TableDelta is PresentationMacLearned { Mac: "00:00:00:00:00:01", PortInterfaceId: "sw1:port1" });
        Assert.Contains(snapshot.Events, traceEvent => traceEvent.Kind == "ping_completed" && traceEvent.Ping?.RttNs == "8700");

        var icmpTransmission = Assert.Single(snapshot.Events, traceEvent => traceEvent.TraceSeq == "17");
        var icmpEthernet = Assert.IsType<PresentationEthernetPdu>(Assert.IsType<PresentationPduObservation>(icmpTransmission.Pdu).Pdu);
        var ipv4Payload = Assert.IsType<PresentationIpv4EthernetPayload>(icmpEthernet.Payload);
        Assert.Equal("192.168.1.10", ipv4Payload.SourceIp);
        Assert.Equal("192.168.1.20", ipv4Payload.DestinationIp);
        Assert.Equal("64", ipv4Payload.Ttl);
    }

    [Fact]
    public void E1S2Cold_ExportIsDeterministic()
    {
        var first = E1S2ColdPresentationExporter.Export().ToJson();
        var second = E1S2ColdPresentationExporter.Export().ToJson();

        Assert.Equal(first, second);
    }
}
