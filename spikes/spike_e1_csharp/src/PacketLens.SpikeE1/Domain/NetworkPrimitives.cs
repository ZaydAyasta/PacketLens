namespace PacketLens.SpikeE1.Domain;

public readonly record struct Endpoint(string Device, string InterfaceName)
{
    public static Endpoint Of(string device, string interfaceName) => new(device, interfaceName);
}

public readonly record struct MacAddress(byte A, byte B, byte C, byte D, byte E, byte F) : IComparable<MacAddress>
{
    public static MacAddress Broadcast { get; } = new(255, 255, 255, 255, 255, 255);
    public static MacAddress Zero { get; } = new(0, 0, 0, 0, 0, 0);

    public static MacAddress Parse(string value)
    {
        var parts = value.Split(':');
        if (parts.Length != 6)
        {
            throw new FormatException("A MAC address must contain six octets.");
        }

        return new MacAddress(
            Convert.ToByte(parts[0], 16), Convert.ToByte(parts[1], 16), Convert.ToByte(parts[2], 16),
            Convert.ToByte(parts[3], 16), Convert.ToByte(parts[4], 16), Convert.ToByte(parts[5], 16));
    }

    public int CompareTo(MacAddress other)
    {
        var comparison = A.CompareTo(other.A);
        if (comparison != 0) return comparison;
        comparison = B.CompareTo(other.B);
        if (comparison != 0) return comparison;
        comparison = C.CompareTo(other.C);
        if (comparison != 0) return comparison;
        comparison = D.CompareTo(other.D);
        if (comparison != 0) return comparison;
        comparison = E.CompareTo(other.E);
        return comparison != 0 ? comparison : F.CompareTo(other.F);
    }
}

public readonly record struct Ipv4Address(byte A, byte B, byte C, byte D) : IComparable<Ipv4Address>
{
    public static Ipv4Address Parse(string value)
    {
        var parts = value.Split('.');
        if (parts.Length != 4)
        {
            throw new FormatException("An IPv4 address must contain four octets.");
        }

        return new Ipv4Address(Convert.ToByte(parts[0]), Convert.ToByte(parts[1]), Convert.ToByte(parts[2]), Convert.ToByte(parts[3]));
    }

    public bool IsInSameSubnet(Ipv4Address other, byte prefixLength)
    {
        var left = ((uint)A << 24) | ((uint)B << 16) | ((uint)C << 8) | D;
        var right = ((uint)other.A << 24) | ((uint)other.B << 16) | ((uint)other.C << 8) | other.D;
        var mask = prefixLength == 0 ? 0u : uint.MaxValue << (32 - prefixLength);
        return (left & mask) == (right & mask);
    }

    public int CompareTo(Ipv4Address other)
    {
        var comparison = A.CompareTo(other.A);
        if (comparison != 0) return comparison;
        comparison = B.CompareTo(other.B);
        if (comparison != 0) return comparison;
        comparison = C.CompareTo(other.C);
        return comparison != 0 ? comparison : D.CompareTo(other.D);
    }
}
