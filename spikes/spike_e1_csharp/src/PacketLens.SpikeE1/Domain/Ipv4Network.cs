namespace PacketLens.SpikeE1.Domain;

public readonly record struct Ipv4Network
{
    public Ipv4Network(Ipv4Address address, int prefixLength)
    {
        Prefix = new Ipv4PrefixLength(prefixLength);
        Address = FromBits(Bits(address) & Prefix.Netmask);
    }

    public Ipv4Address Address { get; }
    public Ipv4PrefixLength Prefix { get; }
    public bool Contains(Ipv4Address address) => (Bits(address) & Prefix.Netmask) == Bits(Address);
    public Ipv4Address? Broadcast => Prefix.Value >= 31 ? null : FromBits(Bits(Address) | ~Prefix.Netmask);
    private static uint Bits(Ipv4Address ip) => ((uint)ip.A << 24) | ((uint)ip.B << 16) | ((uint)ip.C << 8) | ip.D;
    private static Ipv4Address FromBits(uint bits) => new((byte)(bits >> 24), (byte)(bits >> 16), (byte)(bits >> 8), (byte)bits);
}
