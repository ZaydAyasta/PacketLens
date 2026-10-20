namespace PacketLens.SpikeE1.Domain;

public enum ExecutionOutcome
{
    Idle,
    Quiescent,
    TargetTimeReached,
    BudgetExhausted,
    HaltedBySafetyGuard
}

public abstract record SafetyHaltReason;

public sealed record ZeroDelayLoop(ulong AtNs, int EventsAtSameTime) : SafetyHaltReason;

public sealed record QueueCapacityExceeded(int PendingCount, int Limit) : SafetyHaltReason;

public sealed record ExecutionReport(ExecutionOutcome Outcome, int ProcessedEvents, ulong CurrentTimeNs, SafetyHaltReason? SafetyHaltReason = null);
