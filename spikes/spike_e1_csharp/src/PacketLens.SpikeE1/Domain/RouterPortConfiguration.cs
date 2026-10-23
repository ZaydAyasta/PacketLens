namespace PacketLens.SpikeE1.Domain;

public sealed record RouterPortConfiguration(string Name, MacAddress Mac, Ipv4InterfaceAddress Ipv4, bool AdminUp, Ipv4LinkMode Mode = Ipv4LinkMode.MultiAccess);
