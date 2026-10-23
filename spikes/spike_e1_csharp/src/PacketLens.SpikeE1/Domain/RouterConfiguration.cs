namespace PacketLens.SpikeE1.Domain;

public sealed record RouteEntry(Ipv4Network Network, Ipv4Address? NextHop, string EgressInterface, uint Metric, bool IsConnected);

// This model owns configuration, not link state or packet processing.
public sealed class RouterConfiguration
{
    private readonly Dictionary<string,Ipv4InterfaceAddress> _interfaces;
    private readonly List<RouteEntry> _staticRoutes = [];

    public RouterConfiguration(IReadOnlyDictionary<string,Ipv4InterfaceAddress> interfaces)
    {
        if (interfaces.Count < 2 || interfaces.Any(pair => string.IsNullOrWhiteSpace(pair.Key) || pair.Value is null))
            throw new NetworkConfigurationException(NetworkConfigurationError.InvalidRouterConfiguration);
        if (interfaces.Values.Select(value => value.Network).Distinct().Count() != interfaces.Count)
            throw new NetworkConfigurationException(NetworkConfigurationError.DuplicateConnectedPrefix);
        _interfaces = new Dictionary<string,Ipv4InterfaceAddress>(interfaces,StringComparer.Ordinal);
    }

    public IReadOnlyList<RouteEntry> Routes => Array.AsReadOnly(_interfaces.OrderBy(pair => pair.Key,StringComparer.Ordinal)
        .Select(pair => new RouteEntry(pair.Value.Network,null,pair.Key,0,true)).Concat(_staticRoutes).ToArray());

    public void ConfigureInterface(string name, Ipv4InterfaceAddress value)
    {
        if (!_interfaces.ContainsKey(name)) throw new NetworkConfigurationException(NetworkConfigurationError.UnknownInterface);
        var proposed = new Dictionary<string,Ipv4InterfaceAddress>(_interfaces,StringComparer.Ordinal) { [name] = value };
        var candidate = new RouterConfiguration(proposed);
        foreach (var route in _staticRoutes) candidate.AddStaticRoute(route.Network,route.NextHop,route.EgressInterface,route.Metric);
        _interfaces[name] = value;
    }

    public void AddStaticRoute(Ipv4Network network, Ipv4Address? nextHop, string egressInterface, uint metric)
    {
        if (!_interfaces.TryGetValue(egressInterface,out var port)) throw new NetworkConfigurationException(NetworkConfigurationError.UnknownInterface);
        if (_interfaces.Values.Any(value => value.Network == network)) throw new NetworkConfigurationException(NetworkConfigurationError.PrefixOverlapsConnectedNetwork);
        if (_staticRoutes.Any(route => route.Network == network)) throw new NetworkConfigurationException(NetworkConfigurationError.AmbiguousRouteConfig);
        if (nextHop is { } hop)
        {
            if (!Ipv4InterfaceAddress.IsUnicast(hop) || _interfaces.Values.Any(value => value.Address == hop))
                throw new NetworkConfigurationException(NetworkConfigurationError.InvalidNextHop);
            if (!port.Network.Contains(hop)) throw new NetworkConfigurationException(NetworkConfigurationError.NextHopNotDirectlyConnected);
            if (!port.IsUsableOnLink(hop))
                throw new NetworkConfigurationException(NetworkConfigurationError.InvalidNextHop);
        }
        _staticRoutes.Add(new RouteEntry(network,nextHop,egressInterface,metric,false));
    }

    public RouteEntry? LookupRoute(Ipv4Address target, Func<string,bool> isOperational)
    {
        RouteEntry? best = null;
        foreach (var candidate in Routes)
            if (candidate.Network.Contains(target) && isOperational(candidate.EgressInterface)
                && (best is null || candidate.Network.Prefix.Value > best.Network.Prefix.Value)) best = candidate;
        return best;
    }
}
