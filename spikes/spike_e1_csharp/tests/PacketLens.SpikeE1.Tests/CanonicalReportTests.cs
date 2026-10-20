using System.Security.Cryptography;
using System.Text;
using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class CanonicalReportTests
{
    [Fact]
    public void E1S2_CanonicalReport_IsIdenticalAcrossOneHundredIndependentRuns()
    {
        var jsonReports = Enumerable.Range(0, 100).Select(_ => ExecuteScenario2()).ToArray();
        var hashes = jsonReports
            .Select(json => Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(json))))
            .Distinct(StringComparer.Ordinal)
            .ToArray();

        Assert.Single(jsonReports.Distinct(StringComparer.Ordinal));
        Assert.Single(hashes);
        Assert.Contains("\"scenario_id\":\"E1-S2\"", jsonReports[0]);
        Assert.Contains("\"diagnostics\":{\"total_internal_events_processed\":", jsonReports[0]);
        Assert.Contains("\"seq\":1,\"at_ns\":0,\"kind\":\"ArpPacketQueued\",\"details\":\"device=pc1;target=192.168.1.20;packet=ipv4(", jsonReports[0]);
        Assert.Contains("\"kind\":\"FrameTransmitted\",\"details\":\"link=link1;from=pc1:eth0;to=sw1:port1;arrives=1000;frame=eth(", jsonReports[0]);
        Assert.Contains("port: InterfaceName(\\\"port1\\\"), expires_at: VirtualTime(300000001000)", jsonReports[0]);
    }

    private static string ExecuteScenario2()
    {
        var engine = E1Topology.CreateBase();
        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 1);
        var cold = engine.RunUntilQuiescent(100);
        var advance = engine.AdvanceUntil(10_000, 100);
        engine.SendPing("pc1", E1Topology.Pc2Ip, 1, 2);
        var warm = engine.RunUntilQuiescent(100);

        return ScenarioCanonicalReport
            .Create("E1-S2", warm, cold.ProcessedEvents + advance.ProcessedEvents + warm.ProcessedEvents, engine.Trace)
            .ToCanonicalJson();
    }
}
