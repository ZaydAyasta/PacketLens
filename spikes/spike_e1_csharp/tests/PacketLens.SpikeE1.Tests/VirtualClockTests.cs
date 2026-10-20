using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class VirtualClockTests
{
    [Fact]
    public void AdvanceUntil_WithoutEvents_MovesClockToExactTarget()
    {
        var engine = new SimulationEngine();

        var report = engine.AdvanceUntil(77, 0);

        Assert.Equal(ExecutionOutcome.TargetTimeReached, report.Outcome);
        Assert.Equal(77UL, report.CurrentTimeNs);
        Assert.Equal(0, report.ProcessedEvents);
    }

    [Fact]
    public void AdvanceUntil_TargetInPast_ThrowsExplicitDomainErrorWithoutMovingClock()
    {
        var engine = new SimulationEngine();
        engine.AdvanceUntil(77, 0);

        var exception = Assert.Throws<SimulationException>(() => engine.AdvanceUntil(76, 1));

        Assert.Equal(SimulationErrorCode.TargetTimeInPast, exception.Code);
        Assert.Equal(77UL, engine.CurrentTimeNs);
    }
}
