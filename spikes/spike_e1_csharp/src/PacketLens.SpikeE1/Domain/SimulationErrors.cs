namespace PacketLens.SpikeE1.Domain;

public enum SimulationErrorCode
{
    TargetTimeInPast,
    Cancelled,
    TimeOverflow,
    SequenceOverflow,
    EpochOverflow,
    LinkInstanceOverflow,
    EngineBusy
}

public sealed class SimulationException(SimulationErrorCode code) : Exception(code.ToString())
{
    public SimulationErrorCode Code { get; } = code;
}
