using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using Xunit;

namespace PacketLens.SpikeE1.Tests;

public sealed class E21ConfigurationTests
{
    private static HostIpv4Settings Settings(string address, string? gateway = null) =>
        new(new Ipv4InterfaceAddress(Ipv4Address.Parse(address),24),gateway is null ? null : Ipv4Address.Parse(gateway));

    [Fact]
    public void Cfg01EngineConfigurationDoesNotEmitOrAdvance()
    {
        var engine = new SimulationEngine();
        engine.CreateHost("host",["eth0"]);
        var config = Settings("192.168.1.10","192.168.1.1");
        engine.ConfigureHostIpv4(Endpoint.Of("host","eth0"),MacAddress.Parse("02:00:00:00:00:01"),config,true);
        Assert.Equal(config,engine.HostIpv4Configuration("host"));
        Assert.Empty(engine.Trace.Records);
        Assert.Equal(0UL,engine.CurrentTimeNs);
    }

    [Fact]
    public void Cfg01InvalidLegacyUpdatePreservesGatewayAndInterface()
    {
        var engine = new SimulationEngine();
        engine.CreateHost("host",["eth0"]);
        var endpoint = Endpoint.Of("host","eth0");
        var mac = MacAddress.Parse("02:00:00:00:00:01");
        var config = Settings("192.168.1.10","192.168.1.1");
        engine.ConfigureHostIpv4(endpoint,mac,config,true);
        Assert.Equal(NetworkConfigurationError.InvalidGateway,Assert.Throws<NetworkConfigurationException>(() => engine.ConfigureInterface(endpoint,mac,Ipv4Address.Parse("192.168.2.10"),24,true)).Code);
        Assert.Throws<ArgumentException>(() => engine.ConfigureInterface(endpoint,mac,null,24,true));
        Assert.Equal(config,engine.HostIpv4Configuration("host"));
        Assert.Empty(engine.Trace.Records);
        Assert.Equal(0UL,engine.CurrentTimeNs);
    }

    [Fact]
    public void Cfg01RejectsNonhostAndAmbiguousHost()
    {
        var engine = new SimulationEngine();
        engine.CreateL2Switch("switch",["p1"]);
        engine.CreateHost("host",["eth0","eth1"]);
        var mac = MacAddress.Parse("02:00:00:00:00:01");
        engine.ConfigureInterface(Endpoint.Of("host","eth1"),mac,Ipv4Address.Parse("192.168.1.20"),24,true);
        foreach (var endpoint in new[] { Endpoint.Of("switch","p1"), Endpoint.Of("host","eth0"), Endpoint.Of("host","missing") })
            Assert.Throws<ArgumentException>(() => engine.ConfigureHostIpv4(endpoint,mac,Settings("192.168.1.10"),true));
        Assert.Null(engine.HostIpv4Configuration("host"));
        Assert.Empty(engine.Trace.Records);
    }

    [Fact]
    public void Cfg02BusyGuardIncludesLegacyInterfaceEntryPoint()
    {
        var engine = new SimulationEngine();
        engine.CreateHost("host",["eth0"]);
        engine.CreateHost("peer",["eth0"]);
        var mac = MacAddress.Parse("02:00:00:00:00:01");
        engine.ConfigureHostIpv4(Endpoint.Of("host","eth0"),mac,Settings("192.168.1.10"),true);
        engine.ConfigureInterface(Endpoint.Of("peer","eth0"),MacAddress.Parse("02:00:00:00:00:02"),Ipv4Address.Parse("192.168.1.20"),24,true);
        engine.ConnectLink("link",Endpoint.Of("host","eth0"),Endpoint.Of("peer","eth0"));
        engine.SendPing("host",Ipv4Address.Parse("192.168.1.20"),1,1);
        var before = engine.Trace.Records.ToArray();
        Assert.Equal(SimulationErrorCode.EngineBusy,Assert.Throws<SimulationException>(() => engine.ConfigureHostIpv4(Endpoint.Of("host","eth0"),mac,Settings("192.168.1.11"),true)).Code);
        Assert.Equal(SimulationErrorCode.EngineBusy,Assert.Throws<SimulationException>(() => engine.ConfigureInterface(Endpoint.Of("host","eth0"),mac,Ipv4Address.Parse("192.168.1.11"),24,true)).Code);
        Assert.Equal(Settings("192.168.1.10"),engine.HostIpv4Configuration("host"));
        Assert.Equal(before,engine.Trace.Records);
        Assert.Equal(0UL,engine.CurrentTimeNs);
        engine.RunUntilQuiescent(100);
        var completedTrace = engine.Trace.Records.ToArray();
        engine.ConfigureHostIpv4(Endpoint.Of("host","eth0"),mac,Settings("192.168.1.11"),true);
        Assert.Equal(Settings("192.168.1.11"),engine.HostIpv4Configuration("host"));
        Assert.Equal(completedTrace,engine.Trace.Records);
    }
}
