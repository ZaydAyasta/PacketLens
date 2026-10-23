using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;

internal static class E21RouterProbe
{
    private static Ipv4Address Ip(string value) => Ipv4Address.Parse(value);
    private static MacAddress Mac(int value) => MacAddress.Parse($"02:00:00:00:00:{value:x2}");
    private static RouterPortConfiguration[] Ports() => [new("a",Mac(1),new(Ip("192.168.1.1"),24),true),new("b",Mac(2),new(Ip("172.16.1.1"),24),true)];
    private static string Error(Action action)
    {
        try { action(); return "ok"; }
        catch (NetworkConfigurationException e) { return e.Code.ToString(); }
        catch (SimulationException e) { return e.Code.ToString(); }
    }
    private static string State(SimulationEngine e,string label)
    {
        string Selected(string ip) { var r=e.LookupRouterRoute("r",Ip(ip)); return r is null ? "none" : $"{r.EgressInterface}/{r.Network.Prefix.Value}"; }
        return $"state|{label}|{(e.InterfaceIsOperational(Endpoint.Of("r","a")) ? 1 : 0)}|{(e.InterfaceIsOperational(Endpoint.Of("r","b")) ? 1 : 0)}|{Selected("10.1.1.1")}|{Selected("192.168.1.10")}|{e.RouterRoutes("r").Count}";
    }
    private static SimulationEngine Pair(int last,int prefix,bool declared,int connection)
    {
        var e=new SimulationEngine();
        foreach (var (host,address,length,mode) in new[] { ("h0",0,31,true), ("h1",last,prefix,declared) })
        {
            e.CreateHost(host,["p"]); if (mode) e.SetInterfaceIpv4Mode(Endpoint.Of(host,"p"),Ipv4LinkMode.PointToPoint);
            e.ConfigureInterface(Endpoint.Of(host,"p"),Mac(address+1),Ip($"172.16.0.{address}"),(byte)length,true);
        }
        if (connection==0) e.ConnectLink("wire",Endpoint.Of("h0","p"),Endpoint.Of("h1","p"));
        if (connection==2) { e.CreateL2Switch("sw",["a","b"]); e.ConnectLink("left",Endpoint.Of("h0","p"),Endpoint.Of("sw","a")); e.ConnectLink("right",Endpoint.Of("h1","p"),Endpoint.Of("sw","b")); }
        return e;
    }
    private static string[] Project()
    {
        var rows=new List<string>(); var e=new SimulationEngine(); e.CreateRouter("r",Ports()); e.CreateHost("peer",["x","y"]);
        e.AddRouterStaticRoute("r",new(Ip("10.0.0.0"),8),Ip("172.16.1.2"),"b",1); e.AddRouterStaticRoute("r",new(Ip("0.0.0.0"),0),Ip("192.168.1.2"),"a",99);
        rows.Add(State(e,"registered"));
        e.ConnectLink("left",Endpoint.Of("r","a"),Endpoint.Of("peer","x")); rows.Add(State(e,"left"));
        e.ConnectLink("right",Endpoint.Of("r","b"),Endpoint.Of("peer","y")); rows.Add(State(e,"right"));
        e.SetInterfaceAdminState(Endpoint.Of("r","b"),false); rows.Add(State(e,"down"));
        e.SetInterfaceAdminState(Endpoint.Of("r","b"),true); rows.Add(State(e,"up"));
        e.SetInterfaceAdminState(Endpoint.Of("peer","y"),false); rows.Add(State(e,"peer-down"));
        e.DisconnectLink("right"); rows.Add(State(e,"disconnected"));
        e.ConnectLink("right",Endpoint.Of("r","b"),Endpoint.Of("peer","y")); rows.Add(State(e,"reconnected"));
        rows.Add("reject|duplicate|"+Error(() => e.AddRouterStaticRoute("r",new(Ip("10.1.1.1"),8),null,"a",99)));
        rows.Add("reject|connected|"+Error(() => e.ConfigureInterface(Endpoint.Of("r","b"),Mac(2),Ip("192.168.1.2"),24,false)));
        rows.Add("reject|next-hop|"+Error(() => e.ConfigureInterface(Endpoint.Of("r","b"),Mac(2),Ip("172.17.1.1"),24,false)));
        rows.Add(State(e,"after-rejections"));
        foreach (var (host,last) in new[] { ("tx",10), ("rx",20) }) { e.CreateHost(host,["p"]); e.ConfigureInterface(Endpoint.Of(host,"p"),Mac(last),Ip($"192.168.5.{last}"),24,true); }
        e.ConnectLink("traffic",Endpoint.Of("tx","p"),Endpoint.Of("rx","p")); e.SendPing("tx",Ip("192.168.5.20"),1,1);
        rows.Add("busy|route|"+Error(() => e.AddRouterStaticRoute("r",new(Ip("11.0.0.0"),8),null,"a",0)));
        rows.Add("busy|port|"+Error(() => e.ConfigureInterface(Endpoint.Of("r","b"),Mac(2),Ip("172.17.1.1"),24,false)));
        rows.Add("busy|registration|"+Error(() => e.CreateRouter("new",Ports()))); rows.Add(State(e,"after-busy")); e.RunUntilQuiescent(100);
        rows.Add("idle|"+Error(() => e.AddRouterStaticRoute("r",new(Ip("11.0.0.0"),8),null,"a",0)));
        foreach (var (label,last,prefix,mode,connection) in new[] { ("valid",1,31,true,0),("same-ip",0,31,true,0),("wrong-pair",2,31,true,0),("wrong-prefix",1,30,true,0),("no-mode",1,31,false,0),("unwired",1,31,true,1),("switch",1,31,true,2) })
            rows.Add($"topology|{label}|"+Error(() => Pair(last,prefix,mode,connection).ValidateIpv4Topology()));
        var pair=Pair(1,31,true,0);
        foreach (var (source,target,id) in new[] { ("h0","172.16.0.1",1), ("h1","172.16.0.0",2) })
        {
            pair.SendPing(source,Ip(target),(ushort)id,1); pair.RunUntilQuiescent(100);
            rows.Add($"direct-ping|{source}|{pair.Trace.Records.OfType<PingCompleted>().Count(p => p.Host==source)}");
        }
        return rows.ToArray();
    }
    internal static void Run()
    {
        var expected=Project();
        for (var i=1;i<100;i++) if (!Project().SequenceEqual(expected)) throw new InvalidOperationException($"Router projection changed at repetition {i}.");
        foreach (var row in expected) Console.WriteLine(row);
    }
}
