using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;
using PacketLens.SpikeE1.Presentation;
using System.Diagnostics;

internal static class Program
{
    private static readonly MacAddress Pc1 = MacAddress.Parse("00:00:00:00:00:01");
    private static readonly MacAddress Pc2 = MacAddress.Parse("00:00:00:00:00:02");
    private static readonly MacAddress Pc3 = MacAddress.Parse("00:00:00:00:00:03");
    private static readonly Ipv4Address Ip1 = Ipv4Address.Parse("192.168.1.10");
    private static readonly Ipv4Address Ip2 = Ipv4Address.Parse("192.168.1.20");

    private static void Main(string[] args)
    {
        var command = args.ElementAtOrDefault(0);
        if (command == "e21-s1")
        {
            E21S1Probe.Run();
            return;
        }
        if (command == "e21-router")
        {
            E21RouterProbe.Run();
            return;
        }
        if (command == "e21-configuration")
        {
            E21ConfigurationProbe.Run();
            return;
        }
        if (command == "reports")
        {
            foreach (var report in new[] { ScenarioS1(), ScenarioS2(), ScenarioS3A(), ScenarioS3B(), ScenarioS4(), ScenarioS5(), ScenarioS6() })
            {
                Console.WriteLine(report);
            }
            return;
        }

        if (command == "presentation-s2-cold")
        {
            Console.WriteLine(E1S2ColdPresentationExporter.Export().ToJson());
            return;
        }

        if (command == "workload" && args.Length == 3 && int.TryParse(args[2], out var iterations) && iterations >= 0)
        {
            Workload(args[1], iterations);
            return;
        }

        throw new ArgumentException("Usage: reports | presentation-s2-cold | workload <s2-cold|s6> <iterations>");
    }

static SimulationEngine CreateBase()
{
    var engine = new SimulationEngine();
    engine.CreateHost("pc1", ["eth0"]);
    engine.CreateHost("pc2", ["eth0"]);
    engine.CreateHost("pc3", ["eth0"]);
    engine.CreateL2Switch("sw1", ["port1", "port2", "port3", "port4"]);
    engine.ConfigureInterface(Endpoint.Of("pc1", "eth0"), Pc1, Ip1, 24, true);
    engine.ConfigureInterface(Endpoint.Of("pc2", "eth0"), Pc2, Ip2, 24, true);
    engine.ConfigureInterface(Endpoint.Of("pc3", "eth0"), Pc3, Ipv4Address.Parse("192.168.1.30"), 24, true);
    foreach (var port in new[] { "port1", "port2", "port3", "port4" }) engine.ConfigureInterface(Endpoint.Of("sw1", port), MacAddress.Zero, null, 0, true);
    engine.ConnectLink("link1", Endpoint.Of("pc1", "eth0"), Endpoint.Of("sw1", "port1"));
    engine.ConnectLink("link2", Endpoint.Of("pc2", "eth0"), Endpoint.Of("sw1", "port2"));
    engine.ConnectLink("link3", Endpoint.Of("pc3", "eth0"), Endpoint.Of("sw1", "port3"));
    return engine;
}

static SimulationEngine CreateLoop()
{
    var engine = new SimulationEngine(500);
    engine.CreateHost("pc1", ["eth0"]);
    engine.CreateL2Switch("sw1", ["port1", "port2", "port3", "port4"]);
    engine.CreateL2Switch("sw2", ["port1", "port2", "port3", "port4"]);
    engine.ConfigureInterface(Endpoint.Of("pc1", "eth0"), Pc1, Ip1, 24, true);
    foreach (var sw in new[] { "sw1", "sw2" })
    foreach (var port in new[] { "port1", "port2", "port3", "port4" }) engine.ConfigureInterface(Endpoint.Of(sw, port), MacAddress.Zero, null, 0, true);
    engine.ConnectLink("link1", Endpoint.Of("pc1", "eth0"), Endpoint.Of("sw1", "port1"));
    engine.ConnectLink("link_loop_a", Endpoint.Of("sw1", "port2"), Endpoint.Of("sw2", "port2"));
    engine.ConnectLink("link_loop_b", Endpoint.Of("sw1", "port3"), Endpoint.Of("sw2", "port3"));
    return engine;
}

static string Finish(string id, SimulationEngine engine, ExecutionReport report) => ScenarioCanonicalReport.Create(id, report, report.ProcessedEvents, engine.Trace).ToCanonicalJson();

static string ScenarioS1()
{
    var engine = CreateBase(); engine.SendPing("pc1", Ip2, 1, 1);
    return Finish("E1-S1", engine, engine.AdvanceUntil(4_300, 100));
}

static string ScenarioS2()
{
    var engine = CreateBase(); engine.SendPing("pc1", Ip2, 1, 1); engine.RunUntilQuiescent(100);
    engine.AdvanceUntil(10_000, 100); engine.SendPing("pc1", Ip2, 1, 2);
    return Finish("E1-S2", engine, engine.RunUntilQuiescent(100));
}

static string ScenarioS3A()
{
    var engine = CreateBase(); engine.SetInterfaceAdminState(Endpoint.Of("pc2", "eth0"), false); engine.SendPing("pc1", Ip2, 1, 1);
    return Finish("E1-S3A", engine, engine.RunUntilQuiescent(500));
}

static string ScenarioS3B()
{
    var engine = CreateBase(); engine.SendPing("pc1", Ip2, 1, 1); engine.AdvanceUntil(500, 10);
    engine.DisconnectLink("link1"); engine.ConnectLink("link1-new", Endpoint.Of("pc1", "eth0"), Endpoint.Of("sw1", "port1"));
    return Finish("E1-S3B", engine, engine.AdvanceUntil(1_500, 10));
}

static string ScenarioS4()
{
    var engine = CreateBase(); engine.SendPing("pc1", Ip2, 1, 1); engine.RunUntilQuiescent(100); engine.AdvanceUntil(20_000, 100);
    engine.DisconnectLink("link2"); engine.ConnectLink("link4", Endpoint.Of("pc2", "eth0"), Endpoint.Of("sw1", "port4")); engine.SendPing("pc2", Ip1, 2, 1);
    return Finish("E1-S4", engine, engine.RunUntilQuiescent(100));
}

static string ScenarioS5()
{
    var engine = CreateBase(); engine.SendPing("pc1", Ip2, 1, 1); engine.RunUntilQuiescent(100); engine.AdvanceUntil(10_000, 100);
    engine.SendPing("pc1", Ip2, 1, 2); engine.RunUntilQuiescent(100); engine.AdvanceUntil(60_000_004_299, 100);
    engine.AdvanceUntil(60_000_004_300, 100); engine.AdvanceUntil(61_000_000_000, 100); engine.SendPing("pc1", Ip2, 1, 3);
    return Finish("E1-S5", engine, engine.RunUntilQuiescent(100));
}

static string ScenarioS6()
{
    var engine = CreateLoop(); engine.SendPing("pc1", Ipv4Address.Parse("192.168.1.99"), 9, 1);
    return Finish("E1-S6", engine, engine.StepEvents(1_000));
}

static void Workload(string workload, int iterations)
{
    ulong checksum = 0;
    var stopwatch = Stopwatch.StartNew();
    for (var i = 0; i < iterations; i++)
    {
        ExecutionReport report = workload switch
        {
            "s2-cold" => RunS2Cold(),
            "s6" => RunS6(),
            _ => throw new ArgumentException($"Unknown workload: {workload}"),
        };
        checksum = unchecked(checksum + report.CurrentTimeNs + (ulong)report.ProcessedEvents);
    }
    stopwatch.Stop();
    Console.WriteLine($"checksum={checksum};elapsed_ns={stopwatch.ElapsedTicks * 1_000_000_000L / Stopwatch.Frequency}");
}

static ExecutionReport RunS2Cold()
{
    var engine = CreateBase(); engine.SendPing("pc1", Ip2, 1, 1); return engine.RunUntilQuiescent(100);
}

static ExecutionReport RunS6()
{
    var engine = CreateLoop(); engine.SendPing("pc1", Ipv4Address.Parse("192.168.1.99"), 9, 1); return engine.StepEvents(1_000);
}

}
