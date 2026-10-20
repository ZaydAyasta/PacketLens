using System.Globalization;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;

namespace PacketLens.SpikeE1.Domain;

public sealed class ScenarioCanonicalReport
{
    private ScenarioCanonicalReport(string scenarioId, ExecutionReport execution, int totalInternalEventsProcessed, TraceSnapshot trace)
    {
        ScenarioId = scenarioId;
        Execution = execution;
        TotalInternalEventsProcessed = totalInternalEventsProcessed;
        Trace = trace;
    }

    public string ScenarioId { get; }
    public ExecutionReport Execution { get; }
    public int TotalInternalEventsProcessed { get; }
    public TraceSnapshot Trace { get; }

    public static ScenarioCanonicalReport Create(string scenarioId, ExecutionReport execution, int totalInternalEventsProcessed, TraceSnapshot trace)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(scenarioId);
        ArgumentNullException.ThrowIfNull(execution);
        ArgumentNullException.ThrowIfNull(trace);
        ArgumentOutOfRangeException.ThrowIfNegative(totalInternalEventsProcessed);
        return new ScenarioCanonicalReport(scenarioId, execution, totalInternalEventsProcessed, trace);
    }

    public string ToCanonicalJson()
    {
        using var stream = new MemoryStream();
        using (var writer = new Utf8JsonWriter(stream, new JsonWriterOptions { Indented = false, Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping }))
        {
            writer.WriteStartObject();
            writer.WriteString("scenario_id", ScenarioId);
            writer.WriteNumber("final_virtual_time_ns", Execution.CurrentTimeNs);
            writer.WriteString("execution_outcome", Execution.Outcome.ToString());
            writer.WritePropertyName("diagnostics");
            writer.WriteStartObject();
            writer.WriteNumber("total_internal_events_processed", TotalInternalEventsProcessed);
            writer.WriteEndObject();
            writer.WritePropertyName("trace_summary");
            writer.WriteStartObject();
            writer.WriteNumber("total_emitted_ever", Trace.TotalEmittedEver);
            writer.WriteNumber("dropped_by_capacity", Trace.DroppedByCapacity);
            WriteOptionalNumber(writer, "oldest_available_seq", Trace.OldestAvailableSequence);
            WriteOptionalNumber(writer, "newest_available_seq", Trace.NewestAvailableSequence);
            writer.WritePropertyName("records");
            writer.WriteStartArray();
            foreach (var record in Trace.Records)
            {
                writer.WriteStartObject();
                writer.WriteNumber("seq", record.TraceSequence);
                writer.WriteNumber("at_ns", record.AtNs);
                writer.WriteString("kind", ObservableKind(record));
                writer.WriteString("details", ObservableDetails(record));
                writer.WriteEndObject();
            }

            writer.WriteEndArray();
            writer.WriteEndObject();
            writer.WriteEndObject();
        }

        return Encoding.UTF8.GetString(stream.ToArray());
    }

    private static string ObservableKind(TraceEvent trace) => trace switch
    {
        FrameTransmitted => nameof(FrameTransmitted),
        FrameDelivered => nameof(FrameDelivered),
        PacketDropped => nameof(PacketDropped),
        ArpPacketQueued => nameof(ArpPacketQueued),
        TableUpdated => nameof(TableUpdated),
        PingCompleted => nameof(PingCompleted),
        PingFailed => nameof(PingFailed),
        _ => throw new InvalidOperationException("Unknown trace event.")
    };

    private static string ObservableDetails(TraceEvent trace) => trace switch
    {
        FrameTransmitted transmitted => $"link={transmitted.Link};from={EndpointText(transmitted.From)};to={EndpointText(transmitted.To)};arrives={transmitted.ArrivesAtNs};frame={FrameText(transmitted.Frame)}",
        FrameDelivered delivered => $"link={delivered.Link};at={EndpointText(delivered.AtEndpoint)};frame={FrameText(delivered.Frame)}",
        PacketDropped dropped => $"device={dropped.Device};interface={dropped.InterfaceName ?? "-"};reason={dropped.Reason};pdu={DroppedPduText(dropped)}",
        ArpPacketQueued queued => $"device={queued.Device};target={Ipv4Text(queued.TargetIp)};packet={Ipv4Text(queued.Packet)}",
        TableUpdated updated => $"device={updated.Device};update={TableUpdateText(updated.Update)}",
        PingCompleted completed => $"host={completed.Host};target={Ipv4Text(completed.TargetIp)};id={completed.Identifier};seq={completed.Sequence};rtt={completed.RttNs}",
        PingFailed failed => $"host={failed.Host};target={Ipv4Text(failed.TargetIp)};id={failed.Identifier};seq={failed.Sequence};reason={failed.Reason}",
        _ => throw new InvalidOperationException("Unknown trace event.")
    };

    private static string EndpointText(Endpoint endpoint) => $"{endpoint.Device}:{endpoint.InterfaceName}";

    private static string DroppedPduText(PacketDropped dropped) => dropped.EthernetFrame is not null
        ? FrameText(dropped.EthernetFrame)
        : dropped.Ipv4Packet is not null
            ? Ipv4Text(dropped.Ipv4Packet)
            : throw new InvalidOperationException("A dropped packet must retain its PDU.");

    private static string FrameText(EthernetFrame frame) => $"eth(src={MacText(frame.SourceMac)},dst={MacText(frame.DestinationMac)},payload={frame.Payload switch { ArpPacket arp => ArpText(arp), Ipv4Packet ipv4 => Ipv4Text(ipv4), _ => throw new InvalidOperationException("Unknown Ethernet payload.") }})";

    private static string ArpText(ArpPacket packet) => $"arp({packet.Operation},sender_mac={MacText(packet.SenderMac)},sender_ip={Ipv4Text(packet.SenderIp)},target_mac={MacText(packet.TargetMac)},target_ip={Ipv4Text(packet.TargetIp)})";

    private static string Ipv4Text(Ipv4Packet packet) => $"ipv4(src={Ipv4Text(packet.SourceIp)},dst={Ipv4Text(packet.DestinationIp)},ttl={packet.Ttl},payload=Icmp(IcmpPacket {{ kind: {packet.Payload.Kind}, identifier: {packet.Payload.Identifier}, sequence: {packet.Payload.Sequence} }}))";

    private static string TableUpdateText(TableUpdate update) => update switch
    {
        ArpLearned learned => $"ArpLearned {{ ip: {Ipv4DebugText(learned.Ip)}, mac: {MacDebugText(learned.Mac)}, expires_at: VirtualTime({learned.ExpiresAtNs}) }}",
        ArpEntryExpired expired => $"ArpEntryExpired {{ ip: {Ipv4DebugText(expired.Ip)} }}",
        MacLearned learned => $"MacLearned {{ mac: {MacDebugText(learned.Mac)}, port: InterfaceName(\"{learned.Port}\"), expires_at: VirtualTime({learned.ExpiresAtNs}) }}",
        MacEntryExpired expired => $"MacEntryExpired {{ mac: {MacDebugText(expired.Mac)} }}",
        _ => throw new InvalidOperationException("Unknown table update.")
    };

    private static string MacText(MacAddress address) => string.Create(CultureInfo.InvariantCulture, $"{address.A:x2}:{address.B:x2}:{address.C:x2}:{address.D:x2}:{address.E:x2}:{address.F:x2}");

    private static string Ipv4Text(Ipv4Address address) => string.Create(CultureInfo.InvariantCulture, $"{address.A}.{address.B}.{address.C}.{address.D}");

    private static string MacDebugText(MacAddress address) => $"MacAddress([{address.A}, {address.B}, {address.C}, {address.D}, {address.E}, {address.F}])";

    private static string Ipv4DebugText(Ipv4Address address) => $"Ipv4Address([{address.A}, {address.B}, {address.C}, {address.D}])";

    private static void WriteSafetyHalt(Utf8JsonWriter writer, SafetyHaltReason? reason)
    {
        writer.WritePropertyName("safety_halt");
        if (reason is null)
        {
            writer.WriteNullValue();
            return;
        }

        writer.WriteStartObject();
        switch (reason)
        {
            case ZeroDelayLoop zeroDelay:
                writer.WriteString("kind", nameof(ZeroDelayLoop));
                writer.WriteNumber("at_ns", zeroDelay.AtNs);
                writer.WriteNumber("events_at_same_time", zeroDelay.EventsAtSameTime);
                break;
            case QueueCapacityExceeded capacity:
                writer.WriteString("kind", nameof(QueueCapacityExceeded));
                writer.WriteNumber("pending_count", capacity.PendingCount);
                writer.WriteNumber("limit", capacity.Limit);
                break;
            default:
                throw new InvalidOperationException("Unknown safety halt reason.");
        }

        writer.WriteEndObject();
    }

    private static void WriteTraceEvent(Utf8JsonWriter writer, TraceEvent trace)
    {
        writer.WriteStartObject();
        switch (trace)
        {
            case FrameTransmitted transmitted:
                writer.WriteString("kind", nameof(FrameTransmitted));
                writer.WriteString("link", transmitted.Link);
                WriteEndpoint(writer, "from", transmitted.From);
                WriteEndpoint(writer, "to", transmitted.To);
                writer.WriteNumber("arrives_at_ns", transmitted.ArrivesAtNs);
                WriteEthernetFrame(writer, "frame", transmitted.Frame);
                break;
            case FrameDelivered delivered:
                writer.WriteString("kind", nameof(FrameDelivered));
                writer.WriteString("link", delivered.Link);
                WriteEndpoint(writer, "at_endpoint", delivered.AtEndpoint);
                WriteEthernetFrame(writer, "frame", delivered.Frame);
                break;
            case ArpPacketQueued queued:
                writer.WriteString("kind", nameof(ArpPacketQueued));
                writer.WriteString("device", queued.Device);
                writer.WriteString("target_ip", Format(queued.TargetIp));
                WriteIpv4Packet(writer, "packet", queued.Packet);
                break;
            case PacketDropped dropped:
                writer.WriteString("kind", nameof(PacketDropped));
                writer.WriteString("device", dropped.Device);
                WriteOptionalString(writer, "interface", dropped.InterfaceName);
                writer.WritePropertyName("pdu");
                if (dropped.EthernetFrame is not null) WriteEthernetFrameValue(writer, dropped.EthernetFrame);
                else if (dropped.Ipv4Packet is not null) WriteIpv4PacketValue(writer, dropped.Ipv4Packet);
                else writer.WriteNullValue();
                writer.WriteString("reason", dropped.Reason.ToString());
                break;
            case TableUpdated updated:
                writer.WriteString("kind", nameof(TableUpdated));
                writer.WriteString("device", updated.Device);
                WriteTableUpdate(writer, updated.Update);
                break;
            case PingCompleted completed:
                writer.WriteString("kind", nameof(PingCompleted));
                writer.WriteString("host", completed.Host);
                writer.WriteString("target_ip", Format(completed.TargetIp));
                writer.WriteNumber("identifier", completed.Identifier);
                writer.WriteNumber("sequence", completed.Sequence);
                writer.WriteNumber("rtt_ns", completed.RttNs);
                break;
            case PingFailed failed:
                writer.WriteString("kind", nameof(PingFailed));
                writer.WriteString("host", failed.Host);
                writer.WriteString("target_ip", Format(failed.TargetIp));
                writer.WriteNumber("identifier", failed.Identifier);
                writer.WriteNumber("sequence", failed.Sequence);
                writer.WriteString("reason", failed.Reason.ToString());
                break;
            default:
                throw new InvalidOperationException("Unknown trace event.");
        }

        writer.WriteEndObject();
    }

    private static void WriteEndpoint(Utf8JsonWriter writer, string propertyName, Endpoint endpoint)
    {
        writer.WritePropertyName(propertyName);
        writer.WriteStartObject();
        writer.WriteString("device", endpoint.Device);
        writer.WriteString("interface", endpoint.InterfaceName);
        writer.WriteEndObject();
    }

    private static void WriteEthernetFrame(Utf8JsonWriter writer, string propertyName, EthernetFrame frame)
    {
        writer.WritePropertyName(propertyName);
        WriteEthernetFrameValue(writer, frame);
    }

    private static void WriteEthernetFrameValue(Utf8JsonWriter writer, EthernetFrame frame)
    {
        writer.WriteStartObject();
        writer.WriteString("source_mac", Format(frame.SourceMac));
        writer.WriteString("destination_mac", Format(frame.DestinationMac));
        writer.WritePropertyName("payload");
        switch (frame.Payload)
        {
            case ArpPacket arp:
                WriteArpPacketValue(writer, arp);
                break;
            case Ipv4Packet ipv4:
                WriteIpv4PacketValue(writer, ipv4);
                break;
            default:
                throw new InvalidOperationException("Unknown Ethernet payload.");
        }

        writer.WriteEndObject();
    }

    private static void WriteArpPacketValue(Utf8JsonWriter writer, ArpPacket packet)
    {
        writer.WriteStartObject();
        writer.WriteString("kind", nameof(ArpPacket));
        writer.WriteString("operation", packet.Operation.ToString());
        writer.WriteString("sender_mac", Format(packet.SenderMac));
        writer.WriteString("sender_ip", Format(packet.SenderIp));
        writer.WriteString("target_mac", Format(packet.TargetMac));
        writer.WriteString("target_ip", Format(packet.TargetIp));
        writer.WriteEndObject();
    }

    private static void WriteIpv4Packet(Utf8JsonWriter writer, string propertyName, Ipv4Packet packet)
    {
        writer.WritePropertyName(propertyName);
        WriteIpv4PacketValue(writer, packet);
    }

    private static void WriteIpv4PacketValue(Utf8JsonWriter writer, Ipv4Packet packet)
    {
        writer.WriteStartObject();
        writer.WriteString("kind", nameof(Ipv4Packet));
        writer.WriteString("source_ip", Format(packet.SourceIp));
        writer.WriteString("destination_ip", Format(packet.DestinationIp));
        writer.WriteNumber("ttl", packet.Ttl);
        writer.WritePropertyName("payload");
        writer.WriteStartObject();
        writer.WriteString("kind", nameof(IcmpPacket));
        writer.WriteString("type", packet.Payload.Kind.ToString());
        writer.WriteNumber("identifier", packet.Payload.Identifier);
        writer.WriteNumber("sequence", packet.Payload.Sequence);
        writer.WriteEndObject();
        writer.WriteEndObject();
    }

    private static void WriteTableUpdate(Utf8JsonWriter writer, TableUpdate update)
    {
        writer.WritePropertyName("update");
        writer.WriteStartObject();
        switch (update)
        {
            case ArpLearned learned:
                writer.WriteString("kind", nameof(ArpLearned));
                writer.WriteString("ip", Format(learned.Ip));
                writer.WriteString("mac", Format(learned.Mac));
                writer.WriteNumber("expires_at_ns", learned.ExpiresAtNs);
                break;
            case ArpEntryExpired expired:
                writer.WriteString("kind", nameof(ArpEntryExpired));
                writer.WriteString("ip", Format(expired.Ip));
                break;
            case MacLearned learned:
                writer.WriteString("kind", nameof(MacLearned));
                writer.WriteString("mac", Format(learned.Mac));
                writer.WriteString("port", learned.Port);
                writer.WriteNumber("expires_at_ns", learned.ExpiresAtNs);
                break;
            case MacEntryExpired expired:
                writer.WriteString("kind", nameof(MacEntryExpired));
                writer.WriteString("mac", Format(expired.Mac));
                break;
            default:
                throw new InvalidOperationException("Unknown table update.");
        }

        writer.WriteEndObject();
    }

    private static void WriteOptionalNumber(Utf8JsonWriter writer, string propertyName, ulong? value)
    {
        if (value is ulong number) writer.WriteNumber(propertyName, number);
        else writer.WriteNull(propertyName);
    }

    private static void WriteOptionalString(Utf8JsonWriter writer, string propertyName, string? value)
    {
        if (value is not null) writer.WriteString(propertyName, value);
        else writer.WriteNull(propertyName);
    }

    private static string Format(MacAddress address) => string.Create(CultureInfo.InvariantCulture, $"{address.A:X2}:{address.B:X2}:{address.C:X2}:{address.D:X2}:{address.E:X2}:{address.F:X2}");

    private static string Format(Ipv4Address address) => string.Create(CultureInfo.InvariantCulture, $"{address.A}.{address.B}.{address.C}.{address.D}");
}
