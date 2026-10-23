using PacketLens.SpikeE1.Domain;

namespace PacketLens.SpikeE1.Application;

public sealed partial class SimulationEngine
{
    public void SetInterfaceIpv4Mode(Endpoint endpoint, Ipv4LinkMode mode)
    {
        EnsureIpv4ConfigurationIdle();
        if (!Enum.IsDefined(mode)) throw new ArgumentOutOfRangeException(nameof(mode));
        FindInterface(endpoint).Ipv4Mode=mode;
    }

    public void ValidateIpv4Topology()
    {
        foreach (var (name,node) in _devices)
            foreach (var (portName,port) in node.Interfaces)
            {
                if (port.Ipv4?.PrefixLength != 31) continue;
                var endpoint=Endpoint.Of(name,portName);
                if (!ValidPointToPoint(endpoint,node,port))
                    throw new NetworkConfigurationException(NetworkConfigurationError.InvalidPointToPointTopology);
            }
    }

    private bool ValidPointToPoint(Endpoint endpoint, DeviceState node, InterfaceState port)
    {
        if ((!node.IsHost && node.Router is null) || port.Ipv4Mode != Ipv4LinkMode.PointToPoint
            || port.LinkInstance is not { } instance || !_linkNamesByInstance.TryGetValue(instance,out var linkName)
            || !_links.TryGetValue(linkName,out var link) || link.Instance != instance) return false;
        Endpoint peer;
        if (link.A == endpoint) peer=link.B;
        else if (link.B == endpoint) peer=link.A;
        else return false;
        var peerNode=_devices[peer.Device];
        var peerPort=FindInterface(peer);
        if ((!peerNode.IsHost && peerNode.Router is null) || peerPort.LinkInstance != instance
            || peerPort.Ipv4Mode != Ipv4LinkMode.PointToPoint || peerPort.Ipv4?.PrefixLength != 31) return false;
        var local=port.Ipv4!.Address; var remote=peerPort.Ipv4.Address;
        return local != remote && Ipv4InterfaceAddress.IsUnicast(local) && Ipv4InterfaceAddress.IsUnicast(remote)
            && new Ipv4Network(local,31).Contains(remote);
    }

    public void CreateRouter(string name, IReadOnlyList<RouterPortConfiguration> ports)
    {
        EnsureIpv4ConfigurationIdle();
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        if (_devices.ContainsKey(name)) throw new ArgumentException("Duplicate device.",nameof(name));
        if (ports.Select(p => p.Name).Distinct(StringComparer.Ordinal).Count() != ports.Count)
            throw new ArgumentException("Duplicate interface.",nameof(ports));
        var configuration = new RouterConfiguration(ports.ToDictionary(p => p.Name,p => p.Ipv4,StringComparer.Ordinal));
        var state = new DeviceState(false,ports.Select(p => p.Name).ToArray()) { Router = configuration };
        foreach (var settings in ports)
        {
            var port = state.Interfaces[settings.Name];
            port.Mac=settings.Mac;
            port.Ipv4=new Ipv4Configuration(settings.Ipv4.Address,(byte)settings.Ipv4.Network.Prefix.Value);
            port.AdminUp=settings.AdminUp;
            port.Ipv4Mode=settings.Mode;
        }
        _devices.Add(name,state);
    }

    private RouterConfiguration Router(string name) =>
        _devices.TryGetValue(name,out var state) && state.Router is { } router ? router : throw new ArgumentException("Expected existing router.",nameof(name));

    public IReadOnlyList<RouteEntry> RouterRoutes(string name) => Router(name).Routes;
    public Ipv4InterfaceAddress? InterfaceIpv4(Endpoint endpoint) => FindInterface(endpoint).Ipv4 is { } ip ? new(ip.Address,ip.PrefixLength) : null;

    public bool InterfaceIsOperational(Endpoint endpoint)
    {
        var port=FindInterface(endpoint);
        return port.AdminUp && port.LinkInstance is { } instance
            && _linkNamesByInstance.TryGetValue(instance,out var name)
            && _links.TryGetValue(name,out var link) && link.Instance == instance && (link.A == endpoint || link.B == endpoint);
    }

    public RouteEntry? LookupRouterRoute(string name, Ipv4Address destination) =>
        Router(name).LookupRoute(destination,port => InterfaceIsOperational(Endpoint.Of(name,port)));

    public void AddRouterStaticRoute(string name, Ipv4Network network, Ipv4Address? nextHop, string egress, uint metric)
    {
        EnsureIpv4ConfigurationIdle();
        Router(name).AddStaticRoute(network,nextHop,egress,metric);
    }

    private void ConfigureRouterPort(Endpoint endpoint, MacAddress mac, Ipv4Address? ip, byte prefix, bool adminUp)
    {
        EnsureIpv4ConfigurationIdle();
        if (ip is null) throw new NetworkConfigurationException(NetworkConfigurationError.InvalidInterfaceAddress);
        var value = new Ipv4InterfaceAddress(ip.Value,prefix);
        var port = FindInterface(endpoint);
        Router(endpoint.Device).ConfigureInterface(endpoint.InterfaceName,value);
        port.Ipv4 = new Ipv4Configuration(value.Address,prefix);
        port.Mac = mac;
        port.AdminUp = adminUp;
    }
}
