namespace PacketLens.SpikeE1.Domain;

public readonly record struct Ipv4PrefixLength
{
    public Ipv4PrefixLength(int value)
    {
        if (value is < 0 or > 32) throw new ArgumentOutOfRangeException(nameof(value));
        Value = value;
    }

    public int Value { get; }
    public uint Netmask => Value == 0 ? 0U : uint.MaxValue << (32 - Value);
}
