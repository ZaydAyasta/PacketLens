using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class ArpAndSwitchScenarioTests
{
    [Fact]
    public void E1S1_ArpResolutionThroughSwitch_IsReadyAt4300Nanoseconds()
    {
        var engine = E1Topology.CreateBase();

        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 1);
        var report = engine.AdvanceUntil(4_300, 100);

        Assert.Equal(ExecutionOutcome.TargetTimeReached, report.Outcome);
        Assert.Equal(4_300UL, report.CurrentTimeNs);
        Assert.Equal(E1Topology.Pc2Mac, engine.ArpEntry("pc1", E1Topology.Pc2Ip));
        Assert.Equal(0, engine.PendingArpPacketCount("pc1", E1Topology.Pc2Ip));
        Assert.Equal(2, engine.MacTableEntryCount("sw1"));
        Assert.Contains(engine.Trace.Records, trace => trace is PacketDropped
        {
            AtNs: 2_100UL,
            Device: "pc3",
            InterfaceName: "eth0",
            Reason: DropReason.ArpTargetIpMismatch
        });
    }

    [Fact]
    public void A1_ExternalSendPing_TransmitsInitialArpAtZeroNanoseconds()
    {
        var engine = E1Topology.CreateBase();

        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 1);
        engine.AdvanceUntil(0, 10);

        Assert.Contains(engine.Trace.Records, trace => trace is ArpPacketQueued
        {
            AtNs: 0UL,
            Device: "pc1",
            TargetIp: var targetIp
        } && targetIp == E1Topology.Pc2Ip);
        Assert.Contains(engine.Trace.Records, trace => trace is FrameTransmitted
        {
            AtNs: 0UL,
            Link: "link1",
            From: { Device: "pc1", InterfaceName: "eth0" },
            To: { Device: "sw1", InterfaceName: "port1" },
            ArrivesAtNs: 1_000UL,
            Frame.Payload: ArpPacket { Operation: ArpOperation.Request }
        });
    }

    [Fact]
    public void A1_DeferredTransmit_DoesNotUseAnAdminDownEgressInterface()
    {
        var engine = E1Topology.CreateBase();
        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 1);
        engine.AdvanceUntil(1_000, 100);

        engine.SetInterfaceAdminState(Endpoint.Of("sw1", "port2"), false);
        engine.AdvanceUntil(2_100, 100);

        Assert.DoesNotContain(engine.Trace.Records, trace => trace is FrameTransmitted
        {
            AtNs: 1_100UL,
            From: { Device: "sw1", InterfaceName: "port2" }
        });
        Assert.DoesNotContain(engine.Trace.Records, trace => trace is FrameDelivered
        {
            AtNs: 2_100UL,
            AtEndpoint: { Device: "pc2", InterfaceName: "eth0" }
        });
        var drop = Assert.IsType<PacketDropped>(Assert.Single(engine.Trace.Records, trace => trace is PacketDropped
        {
            AtNs: 1_100UL,
            Device: "sw1",
            InterfaceName: "port2",
            Reason: DropReason.EgressInterfaceDown
        }));
        Assert.NotNull(drop.EthernetFrame);
        Assert.Null(drop.Ipv4Packet);
    }

    [Fact]
    public void A1_DeferredTransmit_RecordsEgressLinkUnavailableWhenCurrentLinkWasRemoved()
    {
        var engine = E1Topology.CreateBase();
        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 1);
        engine.AdvanceUntil(1_000, 100);

        engine.DisconnectLink("link2");
        engine.AdvanceUntil(2_100, 100);

        Assert.DoesNotContain(engine.Trace.Records, trace => trace is FrameTransmitted
        {
            AtNs: 1_100UL,
            From: { Device: "sw1", InterfaceName: "port2" }
        });
        Assert.DoesNotContain(engine.Trace.Records, trace => trace is FrameDelivered
        {
            AtNs: 2_100UL,
            AtEndpoint: { Device: "pc2", InterfaceName: "eth0" }
        });
        var drop = Assert.IsType<PacketDropped>(Assert.Single(engine.Trace.Records, trace => trace is PacketDropped
        {
            AtNs: 1_100UL,
            Device: "sw1",
            InterfaceName: "port2",
            Reason: DropReason.EgressLinkUnavailable
        }));
        Assert.NotNull(drop.EthernetFrame);
        Assert.Null(drop.Ipv4Packet);
    }

    [Fact]
    public void A1_DeferredTransmit_RechecksTheCurrentReconnectedLink()
    {
        var engine = E1Topology.CreateBase();
        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 1);
        engine.AdvanceUntil(1_000, 100);

        engine.DisconnectLink("link2");
        engine.ConnectLink("link2-new", Endpoint.Of("pc2", "eth0"), Endpoint.Of("sw1", "port2"));
        engine.AdvanceUntil(2_100, 100);

        Assert.DoesNotContain(engine.Trace.Records, trace => trace is FrameTransmitted
        {
            AtNs: 1_100UL,
            Link: "link2"
        });
        Assert.DoesNotContain(engine.Trace.Records, trace => trace is PacketDropped
        {
            AtNs: 1_100UL,
            Device: "sw1",
            InterfaceName: "port2"
        });
        Assert.Contains(engine.Trace.Records, trace => trace is FrameTransmitted
        {
            AtNs: 1_100UL,
            Link: "link2-new",
            From: { Device: "sw1", InterfaceName: "port2" },
            To: { Device: "pc2", InterfaceName: "eth0" },
            ArrivesAtNs: 2_100UL
        });
        Assert.Contains(engine.Trace.Records, trace => trace is FrameDelivered
        {
            AtNs: 2_100UL,
            Link: "link2-new",
            AtEndpoint: { Device: "pc2", InterfaceName: "eth0" }
        });
    }

    [Fact]
    public void E1S2_ColdAndWarmPing_CompleteAtNormativeTimes()
    {
        var engine = E1Topology.CreateBase();

        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 1);
        var cold = engine.RunUntilQuiescent(100);

        Assert.Equal(ExecutionOutcome.Quiescent, cold.Outcome);
        Assert.Equal(8_700UL, cold.CurrentTimeNs);
        Assert.Contains(engine.Trace.Records, trace => trace is PingCompleted { Identifier: 1, Sequence: 1, RttNs: 8_700UL });

        engine.AdvanceUntil(10_000, 100);
        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 2);
        var warm = engine.RunUntilQuiescent(100);

        Assert.Equal(ExecutionOutcome.Quiescent, warm.Outcome);
        Assert.Equal(14_300UL, warm.CurrentTimeNs);
        Assert.Contains(engine.Trace.Records, trace => trace is PingCompleted { Identifier: 1, Sequence: 2, RttNs: 4_300UL });
        Assert.DoesNotContain(engine.Trace.Records, trace => trace is FrameDelivered { AtEndpoint.Device: "pc3", AtNs: >= 10_000UL });
    }

    [Fact]
    public void E1S3A_DownIngress_ExhaustsThreeArpAttemptsAtThreeSeconds()
    {
        var engine = E1Topology.CreateBase();
        engine.SetInterfaceAdminState(Endpoint.Of("pc2", "eth0"), false);

        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 1);
        var report = engine.RunUntilQuiescent(500);

        Assert.Equal(ExecutionOutcome.Quiescent, report.Outcome);
        Assert.Equal(3_000_000_000UL, report.CurrentTimeNs);
        Assert.Equal(0, engine.PendingArpPacketCount("pc1", E1Topology.Pc2Ip));
        Assert.Contains(engine.Trace.Records, trace => trace is PacketDropped { Reason: DropReason.IngressInterfaceDown });
        Assert.Contains(engine.Trace.Records, trace => trace is PingFailed { AtNs: 3_000_000_000UL, Reason: PingFailureReason.ArpTimeout });
    }

    [Fact]
    public void E1S3B_ReplacedLink_DropsTheInFlightFrameWithOldIdentity()
    {
        var engine = E1Topology.CreateBase();
        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 1);
        engine.AdvanceUntil(500, 10);

        engine.DisconnectLink("link1");
        engine.ConnectLink("link1-new", Endpoint.Of("pc1", "eth0"), Endpoint.Of("sw1", "port1"));
        engine.AdvanceUntil(1_500, 10);

        Assert.Contains(engine.Trace.Records, trace => trace is PacketDropped { AtNs: 1_000UL, Reason: DropReason.LinkDisconnectedInFlight });
    }

    [Fact]
    public void E1S4_MigratedHost_RelearnsMacOnPort4AndPingCompletesAt24300Nanoseconds()
    {
        var engine = E1Topology.CreateBase();
        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 1);
        engine.RunUntilQuiescent(100);
        engine.AdvanceUntil(20_000, 100);
        engine.DisconnectLink("link2");
        engine.ConnectLink("link4", Endpoint.Of("pc2", "eth0"), Endpoint.Of("sw1", "port4"));

        engine.SendPing("pc2", E1Topology.Pc1Ip, 2, 1);
        var report = engine.RunUntilQuiescent(100);

        Assert.Equal(24_300UL, report.CurrentTimeNs);
        Assert.Equal("port4", engine.MacPort("sw1", E1Topology.Pc2Mac));
        Assert.Equal(2, engine.MacTableEntryCount("sw1"));
        Assert.Contains(engine.Trace.Records, trace => trace is PingCompleted { Host: "pc2", Identifier: 2, RttNs: 4_300UL });
    }

    [Fact]
    public void E1S5_ArpUseDoesNotRenewTtlAndEntryExpiresAtExactTimestamp()
    {
        var engine = E1Topology.CreateBase();
        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 1);
        engine.RunUntilQuiescent(100);
        engine.AdvanceUntil(10_000, 100);
        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 2);
        engine.RunUntilQuiescent(100);

        engine.AdvanceUntil(60_000_004_299, 100);
        Assert.Equal(E1Topology.Pc2Mac, engine.ArpEntry("pc1", E1Topology.Pc2Ip));
        engine.AdvanceUntil(60_000_004_300, 100);

        Assert.Null(engine.ArpEntry("pc1", E1Topology.Pc2Ip));
        Assert.Contains(engine.Trace.Records, trace => trace is TableUpdated { AtNs: 60_000_004_300UL, Device: "pc1", Update: ArpEntryExpired { Ip: var ip } } && ip == E1Topology.Pc2Ip);

        engine.AdvanceUntil(61_000_000_000, 100);
        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 3);
        engine.RunUntilQuiescent(100);

        Assert.Contains(engine.Trace.Records, trace => trace is ArpPacketQueued { AtNs: 61_000_000_000UL, Device: "pc1", TargetIp: var ip } && ip == E1Topology.Pc2Ip);
        Assert.Contains(engine.Trace.Records, trace => trace is FrameTransmitted
        {
            AtNs: 61_000_000_000UL,
            From: { Device: "pc1" },
            Frame.Payload: ArpPacket { Operation: ArpOperation.Request }
        });
    }

    [Fact]
    public void MacExpiryTimer_WithCurrentEpoch_RemovesOnlyTheExpiredCamEntry()
    {
        var engine = E1Topology.CreateBase();
        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 1);
        engine.RunUntilQuiescent(100);

        engine.AdvanceUntil(300_000_005_400, 100);

        Assert.Equal(1, engine.MacTableEntryCount("sw1"));
    }
}

internal static class E1Topology
{
    internal static readonly MacAddress Pc1Mac = MacAddress.Parse("00:00:00:00:00:01");
    internal static readonly MacAddress Pc2Mac = MacAddress.Parse("00:00:00:00:00:02");
    internal static readonly MacAddress Pc3Mac = MacAddress.Parse("00:00:00:00:00:03");
    internal static readonly Ipv4Address Pc1Ip = Ipv4Address.Parse("192.168.1.10");
    internal static readonly Ipv4Address Pc2Ip = Ipv4Address.Parse("192.168.1.20");
    internal static readonly Ipv4Address Pc3Ip = Ipv4Address.Parse("192.168.1.30");

    internal static SimulationEngine CreateBase()
    {
        var engine = new SimulationEngine();
        engine.CreateHost("pc1", ["eth0"]);
        engine.CreateHost("pc2", ["eth0"]);
        engine.CreateHost("pc3", ["eth0"]);
        engine.CreateL2Switch("sw1", ["port1", "port2", "port3", "port4"]);
        engine.ConfigureInterface(Endpoint.Of("pc1", "eth0"), Pc1Mac, Pc1Ip, 24, true);
        engine.ConfigureInterface(Endpoint.Of("pc2", "eth0"), Pc2Mac, Pc2Ip, 24, true);
        engine.ConfigureInterface(Endpoint.Of("pc3", "eth0"), Pc3Mac, Pc3Ip, 24, true);
        foreach (var port in new[] { "port1", "port2", "port3", "port4" })
        {
            engine.ConfigureInterface(Endpoint.Of("sw1", port), MacAddress.Zero, null, 0, true);
        }

        engine.ConnectLink("link1", Endpoint.Of("pc1", "eth0"), Endpoint.Of("sw1", "port1"));
        engine.ConnectLink("link2", Endpoint.Of("pc2", "eth0"), Endpoint.Of("sw1", "port2"));
        engine.ConnectLink("link3", Endpoint.Of("pc3", "eth0"), Endpoint.Of("sw1", "port3"));
        return engine;
    }
}
