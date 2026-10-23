using PacketLens.SpikeE1.Application;
using PacketLens.SpikeE1.Domain;

internal static class E21S1Probe
{
    private static Ipv4Address Ip(string s)=>Ipv4Address.Parse(s);
    private static MacAddress Mac(string s)=>MacAddress.Parse(s);
    private static string Address(Ipv4Address a)=>$"{a.A}.{a.B}.{a.C}.{a.D}";
    private static string MacText(MacAddress a)=>$"{a.A:x2}:{a.B:x2}:{a.C:x2}:{a.D:x2}:{a.E:x2}:{a.F:x2}";
    private static string EndpointText(Endpoint a)=>$"{a.Device}:{a.InterfaceName}";
    private static string Packet(Ipv4Packet p)=>$"ip,{Address(p.SourceIp)},{Address(p.DestinationIp)},{p.Ttl},{p.Payload.Kind},{p.Payload.Identifier},{p.Payload.Sequence}";
    private static string Frame(EthernetFrame f)=>$"eth,{MacText(f.SourceMac)},{MacText(f.DestinationMac)},"+(f.Payload switch {
        Ipv4Packet p=>Packet(p),ArpPacket a=>$"arp,{a.Operation},{Address(a.SenderIp)},{MacText(a.SenderMac)},{Address(a.TargetIp)},{MacText(a.TargetMac)}",_=>throw new InvalidOperationException("Unknown PDU.")});
    private static SimulationEngine Fixture()
    {
        var e=new SimulationEngine();
        void AddHost(string name,string address,string gateway,string mac)
        {e.CreateHost(name,["eth0"]);e.ConfigureHostIpv4(Endpoint.Of(name,"eth0"),Mac(mac),new(new(Ip(address),24),Ip(gateway)),true);}
        void AddSwitch(string sw,int d) {e.CreateL2Switch(sw,["port1","port2"]);for(var p=1;p<=2;p++)e.ConfigureInterface(Endpoint.Of(sw,$"port{p}"),Mac($"02:00:00:00:{d:x2}:{p:x2}"),null,24,true);}
        AddHost("host1","192.168.1.10","192.168.1.1","00:00:00:00:01:01"); AddSwitch("sw1",2);
        e.CreateRouter("r1",[new("eth0",Mac("00:00:00:00:01:fe"),new(Ip("192.168.1.1"),24),true),new("eth1",Mac("00:00:00:00:02:fe"),new(Ip("10.0.0.1"),24),true)]);
        AddSwitch("sw2",4); AddHost("host2","10.0.0.20","10.0.0.1","00:00:00:00:02:02");
        foreach(var(link,a,pa,b,pb)in new[]{("l1","host1","eth0","sw1","port1"),("l2","sw1","port2","r1","eth0"),("l3","r1","eth1","sw2","port1"),("l4","sw2","port2","host2","eth0")})e.ConnectLink(link,Endpoint.Of(a,pa),Endpoint.Of(b,pb));
        return e;
    }
    private static string[] Project()
    {
        var rows=new List<string>();
        foreach(var scenario in new[]{"cold","warm-reverse","concurrent"})
        {
            var e=Fixture();foreach(ushort id in scenario=="concurrent"?new ushort[]{3,1,2}:new ushort[]{1})e.SendPing("host1",Ip("10.0.0.20"),id,1);
            var outcome=e.RunUntilQuiescent(1000).Outcome;
            if(scenario=="warm-reverse"){e.SendPing("host2",Ip("192.168.1.10"),7,1);outcome=e.RunUntilQuiescent(1000).Outcome;}
            var trace=e.Trace;rows.Add($"{scenario}|result|{outcome}|{e.CurrentTimeNs}|{trace.TotalEmittedEver}|{trace.DroppedByCapacity}");
            foreach(var r in trace.Records)
            {
                var detail=r switch {
                    FrameTransmitted f=>$"tx|{f.Link}|{EndpointText(f.From)}|{EndpointText(f.To)}|{f.ArrivesAtNs}|{Frame(f.Frame)}",
                    FrameDelivered f=>$"rx|{f.Link}|{EndpointText(f.AtEndpoint)}|{Frame(f.Frame)}",
                    ArpPacketQueued q=>$"queued|{q.Device}|{Address(q.TargetIp)}|{Packet(q.Packet)}",
                    TableUpdated {Update:ArpLearned a} t=>$"arp|{t.Device}|{Address(a.Ip)}|{MacText(a.Mac)}|{a.ExpiresAtNs}",
                    TableUpdated {Update:MacLearned m} t=>$"cam|{t.Device}|{MacText(m.Mac)}|{m.Port}|{m.ExpiresAtNs}",
                    PingCompleted p=>$"completed|{p.Host}|{Address(p.TargetIp)}|{p.Identifier}|{p.Sequence}|{p.RttNs}",
                    _=>throw new InvalidOperationException("Unexpected non-success observation.")};
                rows.Add($"{scenario}|{r.TraceSequence}|{r.AtNs}|{r.ObservationInterface??"-"}|{detail}");
            }
            foreach(var(host,target)in new[]{("host1","192.168.1.1"),("host2","10.0.0.1")})rows.Add($"{scenario}|host-arp|{host}|{target}|{MacText(e.ArpEntry(host,Ip(target))!.Value)}");
            foreach(var(port,target)in new[]{("eth0","192.168.1.10"),("eth1","10.0.0.20")})rows.Add($"{scenario}|router-arp|{port}|{target}|{MacText(e.RouterArpEntry(Endpoint.Of("r1",port),Ip(target))!.Value)}");
            foreach(var(sw,mac)in new[]{("sw1","00:00:00:00:01:01"),("sw1","00:00:00:00:01:fe"),("sw2","00:00:00:00:02:fe"),("sw2","00:00:00:00:02:02")})rows.Add($"{scenario}|cam-final|{sw}|{mac}|{e.MacPort(sw,Mac(mac))}|{e.MacTableEntryCount(sw)}");
        }
        return rows.ToArray();
    }
    internal static void Run(){var expected=Project();for(var i=1;i<100;i++)if(!Project().SequenceEqual(expected))throw new InvalidOperationException($"Non-deterministic S1 repetition {i}.");foreach(var row in expected)Console.WriteLine(row);}
}
