using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class EngineControlTests
{
    [Fact]
    public void Scheduler_WhenSequenceOverflows_ThrowsExplicitErrorWithoutEnqueuing()
    {
        var engine = new SimulationEngine();
        SimulationTestAccess.SetNextSequence(engine, ulong.MaxValue);

        var exception = Assert.Throws<SimulationException>(() => SimulationTestAccess.ScheduleNoOperation(engine, atNs: 0));

        Assert.Equal(SimulationErrorCode.SequenceOverflow, exception.Code);
        Assert.Equal(ExecutionOutcome.Idle, engine.StepEvents(maxSteps: 1).Outcome);
    }

    [Fact]
    public void SendPing_WhenArpRetryTimeOverflows_ThrowsExplicitErrorWithoutPartialState()
    {
        var engine = E1Topology.CreateBase();
        engine.AdvanceUntil(ulong.MaxValue, maxStepsGuard: 100);

        var exception = Assert.Throws<SimulationException>(() => engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 1));

        Assert.Equal(SimulationErrorCode.TimeOverflow, exception.Code);
        Assert.Equal(ulong.MaxValue, engine.CurrentTimeNs);
        Assert.Equal(0, engine.PendingArpPacketCount("pc1", E1Topology.Pc2Ip));
        Assert.Empty(engine.Trace.Records);
    }

    [Fact]
    public void StepEvents_WhenPendingQueueWouldExceedCapacity_HaltsWithQueueCapacityExceeded()
    {
        var engine = new SimulationEngine();
        for (var index = 0; index <= SimulationEngine.MaxPendingQueueSize; index++)
        {
            SimulationTestAccess.ScheduleNoOperation(engine, atNs: 0);
        }

        var report = engine.StepEvents(maxSteps: 1);

        Assert.Equal(ExecutionOutcome.HaltedBySafetyGuard, report.Outcome);
        var halt = Assert.IsType<QueueCapacityExceeded>(report.SafetyHaltReason);
        Assert.Equal(SimulationEngine.MaxPendingQueueSize + 1, halt.PendingCount);
        Assert.Equal(SimulationEngine.MaxPendingQueueSize, halt.Limit);
        Assert.Equal(0, report.ProcessedEvents);
    }

    [Fact]
    public void StepEvents_MoreThanOneThousandEventsAtOneVirtualTime_HaltsWithZeroDelayLoop()
    {
        var engine = new SimulationEngine();
        for (var index = 0; index <= SimulationEngine.MaxZeroDelayEvents; index++)
        {
            SimulationTestAccess.ScheduleNoOperation(engine, atNs: 0);
        }

        var report = engine.StepEvents(maxSteps: SimulationEngine.MaxZeroDelayEvents + 1);

        Assert.Equal(ExecutionOutcome.HaltedBySafetyGuard, report.Outcome);
        var halt = Assert.IsType<ZeroDelayLoop>(report.SafetyHaltReason);
        Assert.Equal(0UL, halt.AtNs);
        Assert.Equal(SimulationEngine.MaxZeroDelayEvents + 1, halt.EventsAtSameTime);
        Assert.Equal(SimulationEngine.MaxZeroDelayEvents, report.ProcessedEvents);
    }

    [Fact]
    public void RunUntilQuiescent_WhenCancellationIsRequested_ThrowsExplicitCancellationErrorBeforeProcessing()
    {
        var engine = E1Topology.CreateBase();
        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 1);
        engine.RequestCancellation();

        var exception = Assert.Throws<SimulationException>(() => engine.RunUntilQuiescent(100));

        Assert.Equal(SimulationErrorCode.Cancelled, exception.Code);
        Assert.Equal(0UL, engine.CurrentTimeNs);
        engine.ClearCancellation();
        Assert.Equal(8_700UL, engine.RunUntilQuiescent(100).CurrentTimeNs);
    }
}
