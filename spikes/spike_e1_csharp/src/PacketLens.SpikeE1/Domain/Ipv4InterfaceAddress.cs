namespace PacketLens.SpikeE1.Domain;

public enum NetworkConfigurationError
{
    InvalidInterfaceAddress, InvalidGateway, InvalidRouterConfiguration,
    DuplicateConnectedPrefix, UnknownInterface, AmbiguousRouteConfig,
    PrefixOverlapsConnectedNetwork, NextHopNotDirectlyConnected, InvalidNextHop,
    InvalidPointToPointTopology
}

public sealed class NetworkConfigurationException(NetworkConfigurationError code) : Exception(code.ToString())
{
    public NetworkConfigurationError Code { get; } = code;
}

public sealed record Ipv4InterfaceAddress
{
    public Ipv4InterfaceAddress(Ipv4Address address, int prefixLength)
    {
        Address = address;
        Network = new Ipv4Network(address,prefixLength);
        if (!IsUsableOnLink(address)) throw new NetworkConfigurationException(NetworkConfigurationError.InvalidInterfaceAddress);
    }

    public Ipv4Address Address { get; }
    public Ipv4Network Network { get; }

    internal static bool IsUnicast(Ipv4Address address) => address.A != 0 && address.A != 127 && address.A < 224;

    public bool IsUsableOnLink(Ipv4Address address) =>
        IsUnicast(address)
        && Network.Contains(address) && Network.Broadcast != address
        && (Network.Prefix.Value >= 31 || Network.Address != address);
}

public sealed record HostIpv4Settings
{
    public HostIpv4Settings(Ipv4InterfaceAddress address, Ipv4Address? defaultGateway = null)
    {
        ArgumentNullException.ThrowIfNull(address);
        if (defaultGateway is { } gateway && (!address.IsUsableOnLink(gateway) || gateway == address.Address))
            throw new NetworkConfigurationException(NetworkConfigurationError.InvalidGateway);
        Interface = address;
        DefaultGateway = defaultGateway;
    }

    public Ipv4InterfaceAddress Interface { get; }
    public Ipv4Address? DefaultGateway { get; }
}
