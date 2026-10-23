using PacketLens.SpikeE1.Domain;

internal static class E21ConfigurationProbe
{
    private static Ipv4Address Ip(string text) => Ipv4Address.Parse(text);
    private static string Text(Ipv4Address value) => $"{value.A}.{value.B}.{value.C}.{value.D}";

    private static string[] Project()
    {
        var output = new List<string>();
        foreach (var length in Enumerable.Range(0,33))
        {
            var net = new Ipv4Network(Ip("192.168.1.10"),length);
            output.Add($"prefix|{length}|{new Ipv4PrefixLength(length).Netmask:x8}|{Text(net.Address)}");
        }
        var port = new Ipv4InterfaceAddress(Ip("192.168.1.10"),24);
        foreach (var candidate in new[] { "192.168.1.1", "192.168.1.10", "192.168.2.1", "192.168.1.0", "192.168.1.255", "0.0.0.0", "127.0.0.1", "224.0.0.1" })
        {
            string result;
            try { _ = new HostIpv4Settings(port,Ip(candidate)); result = "ok"; }
            catch (NetworkConfigurationException error) { result = error.Code.ToString(); }
            output.Add($"gateway|{candidate}|{result}");
        }
        var config = new RouterConfiguration(new Dictionary<string,Ipv4InterfaceAddress>
        {
            ["eth0"] = new(Ip("192.168.1.1"),24), ["eth1"] = new(Ip("172.16.1.1"),24)
        });
        foreach (var (network,length) in new[] { ("10.0.0.0",8), ("10.1.0.0",16), ("10.1.1.99",24), ("10.1.1.50",32) })
            config.AddStaticRoute(new Ipv4Network(Ip(network),length),Ip("172.16.1.2"),"eth1",999);
        config.AddStaticRoute(new Ipv4Network(Ip("0.0.0.0"),0),Ip("192.168.1.2"),"eth0",0);
        for (var state=0; state<4; state++)
            foreach (var target in new[] { "10.1.1.50", "10.1.1.51", "10.1.2.50", "10.2.2.50", "192.168.1.10", "198.51.100.1" })
            {
                var selected=config.LookupRoute(Ip(target),name => (name == "eth0" && (state & 1) != 0) || (name == "eth1" && (state & 2) != 0));
                output.Add($"lookup|{state}|{target}|" + (selected is null ? "none" : $"{Text(selected.Network.Address)}/{selected.Network.Prefix.Value}|{selected.EgressInterface}"));
            }
        foreach (var hop in new[] { "224.0.0.1", "192.168.1.1", "172.17.1.2", "172.16.1.255" })
        {
            try { config.AddStaticRoute(new Ipv4Network(Ip("203.0.113.0"),24),Ip(hop),"eth1",0); throw new InvalidOperationException("Invalid route accepted."); }
            catch (NetworkConfigurationException error) { output.Add($"next-hop|{hop}|{error.Code}"); }
        }
        return output.ToArray();
    }

    internal static void Run()
    {
        var expected = Project();
        foreach (var iteration in Enumerable.Range(1,99))
            if (!Project().SequenceEqual(expected)) throw new InvalidOperationException($"Non-deterministic configuration iteration {iteration}.");
        foreach (var row in expected) Console.WriteLine(row);
    }
}
