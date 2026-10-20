using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class SchedulerAndTraceTests
{
    [Fact]
    public void E1S6_L2Loop_ExhaustsBudgetWithoutSafetyHaltAndTruncatesTrace()
    {
        var engine = new SimulationEngine(traceCapacity: 500);
        engine.CreateHost("pc1", ["eth0"]);
        engine.CreateL2Switch("sw1", ["port1", "port2", "port3", "port4"]);
        engine.CreateL2Switch("sw2", ["port1", "port2", "port3", "port4"]);
        engine.ConfigureInterface(Endpoint.Of("pc1", "eth0"), E1Topology.Pc1Mac, E1Topology.Pc1Ip, 24, true);
        foreach (var device in new[] { "sw1", "sw2" })
        {
            foreach (var port in new[] { "port1", "port2", "port3", "port4" })
            {
                engine.ConfigureInterface(Endpoint.Of(device, port), MacAddress.Zero, null, 0, true);
            }
        }

        engine.ConnectLink("link1", Endpoint.Of("pc1", "eth0"), Endpoint.Of("sw1", "port1"));
        engine.ConnectLink("loop-a", Endpoint.Of("sw1", "port2"), Endpoint.Of("sw2", "port2"));
        engine.ConnectLink("loop-b", Endpoint.Of("sw1", "port3"), Endpoint.Of("sw2", "port3"));
        engine.SendPing("pc1", Ipv4Address.Parse("192.168.1.99"), 9, 1);

        var report = engine.StepEvents(1_000);

        Assert.Equal(ExecutionOutcome.BudgetExhausted, report.Outcome);
        Assert.Equal(1_000, report.ProcessedEvents);
        Assert.Equal(500, engine.Trace.Records.Count);
        Assert.True(engine.Trace.DroppedByCapacity > 0);
        Assert.Equal(engine.Trace.TotalEmittedEver - 500, engine.Trace.DroppedByCapacity);
        Assert.Equal(engine.Trace.DroppedByCapacity + 1, engine.Trace.OldestAvailableSequence);
        Assert.Equal(engine.Trace.TotalEmittedEver, engine.Trace.NewestAvailableSequence);
        Assert.Equal(
            Enumerable.Range(0, engine.Trace.Records.Count)
                .Select(offset => engine.Trace.OldestAvailableSequence!.Value + (ulong)offset),
            engine.Trace.Records.Select(record => record.TraceSequence));
    }
}
