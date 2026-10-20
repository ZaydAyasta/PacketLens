using PacketLens.SpikeE1.Presentation;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class PresentationContractTests
{
    [Fact]
    public void Validate_WhenLinkReferencesAnUnknownInterface_ReturnsAContractError()
    {
        var snapshot = SimulationPresentationSnapshot.CreateForValidation(
            "E1-S2",
            "cold",
            new PresentationTopology(
                [new PresentationDevice("pc1", "host"), new PresentationDevice("sw1", "switch")],
                [new PresentationInterface("pc1:eth0", "pc1", "eth0"), new PresentationInterface("sw1:port1", "sw1", "port1")],
                [new PresentationLink("link1", "pc1:eth0", "sw1:port1")]));
        var invalid = snapshot with
        {
            Topology = snapshot.Topology with
            {
                Links = [new PresentationLink("link1", "pc1:eth0", "missing:eth0")]
            }
        };

        var error = Assert.Single(invalid.Validate());

        Assert.Equal(PresentationContractErrorCode.UnknownLinkInterface, error.Code);
        Assert.Equal("link1", error.LinkId);
        Assert.Equal("missing:eth0", error.InterfaceId);
    }
}
