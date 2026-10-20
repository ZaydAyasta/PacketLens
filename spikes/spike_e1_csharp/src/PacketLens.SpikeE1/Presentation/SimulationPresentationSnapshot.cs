using System.Text;
using System.Text.Json;

namespace PacketLens.SpikeE1.Presentation;

/// <summary>
/// Read contract for a presentation client. It is deliberately separate from
/// the domain trace and from ScenarioCanonicalReport (ADR-0002).
/// </summary>
public sealed record SimulationPresentationSnapshot(
    string SchemaVersion,
    string ScenarioId,
    string RunLabel,
    PresentationTopology Topology,
    PresentationExecution Execution,
    TraceRetention TraceRetention,
    IReadOnlyList<PresentationEvent> Events)
{
    public const string CurrentSchemaVersion = "presentation/v1";

    public static SimulationPresentationSnapshot CreateForValidation(string scenarioId, string runLabel, PresentationTopology topology) =>
        new(
            CurrentSchemaVersion,
            scenarioId,
            runLabel,
            topology,
            new PresentationExecution("0", "Idle"),
            new TraceRetention("0", "0", null, null),
            Array.Empty<PresentationEvent>());

    public IReadOnlyList<PresentationContractError> Validate()
    {
        var errors = new List<PresentationContractError>();
        if (!string.Equals(SchemaVersion, CurrentSchemaVersion, StringComparison.Ordinal))
        {
            errors.Add(new(PresentationContractErrorCode.UnsupportedSchema, "schema_version"));
        }

        AddBlankError(errors, "scenario_id", ScenarioId);
        AddBlankError(errors, "run_label", RunLabel);
        var devices = new HashSet<string>(StringComparer.Ordinal);
        foreach (var device in Topology.Devices)
        {
            AddBlankError(errors, "topology.devices[].id", device.Id);
            AddBlankError(errors, "topology.devices[].kind", device.Kind);
            if (!devices.Add(device.Id))
            {
                errors.Add(new(PresentationContractErrorCode.DuplicateDeviceId, "topology.devices[].id", DeviceId: device.Id));
            }
        }

        var interfaces = new HashSet<string>(StringComparer.Ordinal);
        foreach (var networkInterface in Topology.Interfaces)
        {
            AddBlankError(errors, "topology.interfaces[].id", networkInterface.Id);
            AddBlankError(errors, "topology.interfaces[].device_id", networkInterface.DeviceId);
            AddBlankError(errors, "topology.interfaces[].name", networkInterface.Name);
            if (!devices.Contains(networkInterface.DeviceId))
            {
                errors.Add(new(PresentationContractErrorCode.UnknownInterfaceDevice, "topology.interfaces[].device_id", DeviceId: networkInterface.DeviceId, InterfaceId: networkInterface.Id));
            }
            if (!interfaces.Add(networkInterface.Id))
            {
                errors.Add(new(PresentationContractErrorCode.DuplicateInterfaceId, "topology.interfaces[].id", InterfaceId: networkInterface.Id));
            }
        }

        var links = new HashSet<string>(StringComparer.Ordinal);
        foreach (var link in Topology.Links)
        {
            AddBlankError(errors, "topology.links[].id", link.Id);
            if (!links.Add(link.Id))
            {
                errors.Add(new(PresentationContractErrorCode.DuplicateLinkId, "topology.links[].id", LinkId: link.Id));
            }
            if (!interfaces.Contains(link.AInterfaceId))
            {
                errors.Add(new(PresentationContractErrorCode.UnknownLinkInterface, "topology.links[].a_interface_id", LinkId: link.Id, InterfaceId: link.AInterfaceId));
            }
            if (!interfaces.Contains(link.BInterfaceId))
            {
                errors.Add(new(PresentationContractErrorCode.UnknownLinkInterface, "topology.links[].b_interface_id", LinkId: link.Id, InterfaceId: link.BInterfaceId));
            }
            if (string.Equals(link.AInterfaceId, link.BInterfaceId, StringComparison.Ordinal))
            {
                errors.Add(new(PresentationContractErrorCode.SelfReferentialLink, "topology.links[]", LinkId: link.Id));
            }
        }

        AddDecimalError(errors, "execution.final_virtual_time_ns", Execution.FinalVirtualTimeNs);
        AddBlankError(errors, "execution.outcome", Execution.Outcome);
        AddDecimalError(errors, "trace_retention.total_emitted_ever", TraceRetention.TotalEmittedEver);
        AddDecimalError(errors, "trace_retention.dropped_by_capacity", TraceRetention.DroppedByCapacity);
        ValidateRetention(errors);
        ValidateEvents(errors, interfaces, links);
        return errors;
    }

    public string ToJson()
    {
        var errors = Validate();
        if (errors.Count != 0)
        {
            throw new InvalidOperationException($"Cannot serialize an invalid {CurrentSchemaVersion} snapshot: {errors[0].Code}.");
        }

        using var stream = new MemoryStream();
        using (var writer = new Utf8JsonWriter(stream, new JsonWriterOptions { Indented = false }))
        {
            writer.WriteStartObject();
            writer.WriteString("schema_version", SchemaVersion);
            writer.WriteString("scenario_id", ScenarioId);
            writer.WriteString("run_label", RunLabel);
            WriteTopology(writer, Topology);
            writer.WritePropertyName("execution");
            writer.WriteStartObject();
            writer.WriteString("final_virtual_time_ns", Execution.FinalVirtualTimeNs);
            writer.WriteString("outcome", Execution.Outcome);
            writer.WriteEndObject();
            WriteTraceRetention(writer, TraceRetention);
            writer.WritePropertyName("events");
            writer.WriteStartArray();
            foreach (var traceEvent in Events)
            {
                WriteEvent(writer, traceEvent);
            }
            writer.WriteEndArray();
            writer.WriteEndObject();
        }
        return Encoding.UTF8.GetString(stream.ToArray());
    }

    private void ValidateRetention(List<PresentationContractError> errors)
    {
        if (!TryDecimal(TraceRetention.TotalEmittedEver, out var total) || !TryDecimal(TraceRetention.DroppedByCapacity, out var dropped) || dropped > total)
        {
            errors.Add(new(PresentationContractErrorCode.TraceRetentionMismatch, "trace_retention"));
            return;
        }
        if (total - dropped != (ulong)Events.Count)
        {
            errors.Add(new(PresentationContractErrorCode.TraceRetentionMismatch, "trace_retention"));
        }
        var boundsMatch = Events.Count == 0
            ? TraceRetention.OldestAvailableTraceSeq is null && TraceRetention.NewestAvailableTraceSeq is null
            : TraceRetention.OldestAvailableTraceSeq == Events[0].TraceSeq && TraceRetention.NewestAvailableTraceSeq == Events[^1].TraceSeq;
        if (!boundsMatch)
        {
            errors.Add(new(PresentationContractErrorCode.TraceRetentionBoundsMismatch, "trace_retention"));
        }
    }

    private void ValidateEvents(List<PresentationContractError> errors, HashSet<string> interfaces, HashSet<string> links)
    {
        ulong? previousSequence = null;
        ulong? previousTime = null;
        foreach (var traceEvent in Events)
        {
            AddDecimalError(errors, "events[].trace_seq", traceEvent.TraceSeq);
            AddDecimalError(errors, "events[].at_ns", traceEvent.AtNs);
            AddBlankError(errors, "events[].kind", traceEvent.Kind);
            AddBlankError(errors, "events[].actor.device_id", traceEvent.Actor.DeviceId);
            ValidateEndpoint(errors, traceEvent.TraceSeq, traceEvent.Ingress, interfaces);
            ValidateEndpoint(errors, traceEvent.TraceSeq, traceEvent.Egress, interfaces);
            ValidateEndpoint(errors, traceEvent.TraceSeq, traceEvent.Destination, interfaces);
            if (traceEvent.Actor.InterfaceId is { } actorInterface && !interfaces.Contains(actorInterface))
            {
                errors.Add(new(PresentationContractErrorCode.UnknownEventInterface, "events[].actor.interface_id", InterfaceId: actorInterface, TraceSeq: traceEvent.TraceSeq));
            }
            if (traceEvent.LinkId is { } linkId && !links.Contains(linkId))
            {
                errors.Add(new(PresentationContractErrorCode.UnknownEventLink, "events[].link_id", LinkId: linkId, TraceSeq: traceEvent.TraceSeq));
            }
            if (TryDecimal(traceEvent.TraceSeq, out var sequence))
            {
                if (previousSequence is { } prior && sequence != prior + 1)
                {
                    errors.Add(new(PresentationContractErrorCode.NonContiguousTraceSequence, "events[].trace_seq", TraceSeq: traceEvent.TraceSeq));
                }
                previousSequence = sequence;
            }
            if (TryDecimal(traceEvent.AtNs, out var atNs))
            {
                if (previousTime is { } prior && atNs < prior)
                {
                    errors.Add(new(PresentationContractErrorCode.NonMonotonicVirtualTime, "events[].at_ns", TraceSeq: traceEvent.TraceSeq));
                }
                previousTime = atNs;
            }
        }
    }

    private static void ValidateEndpoint(List<PresentationContractError> errors, string traceSeq, PresentationEndpoint? endpoint, HashSet<string> interfaces)
    {
        if (endpoint is not null && !interfaces.Contains(endpoint.InterfaceId))
        {
            errors.Add(new(PresentationContractErrorCode.UnknownEventInterface, "events[].endpoint.interface_id", InterfaceId: endpoint.InterfaceId, TraceSeq: traceSeq));
        }
    }

    private static void AddBlankError(List<PresentationContractError> errors, string field, string value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            errors.Add(new(PresentationContractErrorCode.BlankField, field));
        }
    }

    private static void AddDecimalError(List<PresentationContractError> errors, string field, string value)
    {
        if (!TryDecimal(value, out _))
        {
            errors.Add(new(PresentationContractErrorCode.InvalidDecimal, field));
        }
    }

    private static bool TryDecimal(string value, out ulong parsed)
    {
        if (value.Length > 0 && value.All(char.IsAsciiDigit) && ulong.TryParse(value, out parsed))
        {
            return true;
        }
        parsed = 0;
        return false;
    }

    private static void WriteTopology(Utf8JsonWriter writer, PresentationTopology topology)
    {
        writer.WritePropertyName("topology");
        writer.WriteStartObject();
        writer.WritePropertyName("devices");
        writer.WriteStartArray();
        foreach (var device in topology.Devices)
        {
            writer.WriteStartObject(); writer.WriteString("id", device.Id); writer.WriteString("kind", device.Kind); writer.WriteEndObject();
        }
        writer.WriteEndArray();
        writer.WritePropertyName("interfaces");
        writer.WriteStartArray();
        foreach (var networkInterface in topology.Interfaces)
        {
            writer.WriteStartObject(); writer.WriteString("id", networkInterface.Id); writer.WriteString("device_id", networkInterface.DeviceId); writer.WriteString("name", networkInterface.Name); writer.WriteEndObject();
        }
        writer.WriteEndArray();
        writer.WritePropertyName("links");
        writer.WriteStartArray();
        foreach (var link in topology.Links)
        {
            writer.WriteStartObject(); writer.WriteString("id", link.Id); writer.WriteString("a_interface_id", link.AInterfaceId); writer.WriteString("b_interface_id", link.BInterfaceId); writer.WriteEndObject();
        }
        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    private static void WriteTraceRetention(Utf8JsonWriter writer, TraceRetention retention)
    {
        writer.WritePropertyName("trace_retention");
        writer.WriteStartObject();
        writer.WriteString("total_emitted_ever", retention.TotalEmittedEver);
        writer.WriteString("dropped_by_capacity", retention.DroppedByCapacity);
        WriteOptionalString(writer, "oldest_available_trace_seq", retention.OldestAvailableTraceSeq);
        WriteOptionalString(writer, "newest_available_trace_seq", retention.NewestAvailableTraceSeq);
        writer.WriteEndObject();
    }

    private static void WriteEvent(Utf8JsonWriter writer, PresentationEvent traceEvent)
    {
        writer.WriteStartObject();
        writer.WriteString("trace_seq", traceEvent.TraceSeq);
        writer.WriteString("at_ns", traceEvent.AtNs);
        writer.WriteString("kind", traceEvent.Kind);
        WriteActor(writer, traceEvent.Actor);
        WriteOptionalEndpoint(writer, "ingress", traceEvent.Ingress);
        WriteOptionalEndpoint(writer, "egress", traceEvent.Egress);
        WriteOptionalEndpoint(writer, "destination", traceEvent.Destination);
        WriteOptionalString(writer, "link_id", traceEvent.LinkId);
        writer.WritePropertyName("protocols"); writer.WriteStartArray(); foreach (var protocol in traceEvent.Protocols) writer.WriteStringValue(protocol); writer.WriteEndArray();
        if (traceEvent.Pdu is null) writer.WriteNull("pdu"); else { writer.WritePropertyName("pdu"); WritePduObservation(writer, traceEvent.Pdu); }
        if (traceEvent.TableDelta is null) writer.WriteNull("table_delta"); else { writer.WritePropertyName("table_delta"); WriteTableDelta(writer, traceEvent.TableDelta); }
        WriteOptionalString(writer, "drop_reason", traceEvent.DropReason);
        if (traceEvent.Ping is null) writer.WriteNull("ping"); else { writer.WritePropertyName("ping"); WritePing(writer, traceEvent.Ping); }
        writer.WriteEndObject();
    }

    private static void WriteActor(Utf8JsonWriter writer, PresentationActor actor)
    {
        writer.WritePropertyName("actor"); writer.WriteStartObject(); writer.WriteString("device_id", actor.DeviceId); WriteOptionalString(writer, "interface_id", actor.InterfaceId); writer.WriteEndObject();
    }

    private static void WriteOptionalEndpoint(Utf8JsonWriter writer, string property, PresentationEndpoint? endpoint)
    {
        if (endpoint is null) { writer.WriteNull(property); return; }
        writer.WritePropertyName(property); writer.WriteStartObject(); writer.WriteString("device_id", endpoint.DeviceId); writer.WriteString("interface_id", endpoint.InterfaceId); writer.WriteEndObject();
    }

    private static void WriteOptionalString(Utf8JsonWriter writer, string property, string? value)
    {
        if (value is null) writer.WriteNull(property); else writer.WriteString(property, value);
    }

    private static void WritePduObservation(Utf8JsonWriter writer, PresentationPduObservation observation)
    {
        writer.WriteStartObject(); writer.WriteString("disposition", observation.Disposition); writer.WritePropertyName("pdu"); WritePdu(writer, observation.Pdu); writer.WriteEndObject();
    }

    private static void WritePdu(Utf8JsonWriter writer, PresentationPdu pdu)
    {
        writer.WriteStartObject();
        switch (pdu)
        {
            case PresentationEthernetPdu ethernet:
                writer.WriteString("kind", "ethernet"); writer.WriteString("src_mac", ethernet.SourceMac); writer.WriteString("dst_mac", ethernet.DestinationMac); writer.WritePropertyName("payload"); WriteEthernetPayload(writer, ethernet.Payload); break;
            case PresentationIpv4Pdu ipv4:
                writer.WriteString("kind", "ipv4"); writer.WriteString("src_ip", ipv4.SourceIp); writer.WriteString("dst_ip", ipv4.DestinationIp); writer.WriteString("ttl", ipv4.Ttl); writer.WritePropertyName("payload"); WriteIpv4Payload(writer, ipv4.Payload); break;
            default: throw new InvalidOperationException($"Unsupported presentation PDU {pdu.GetType().Name}.");
        }
        writer.WriteEndObject();
    }

    private static void WriteEthernetPayload(Utf8JsonWriter writer, PresentationEthernetPayload payload)
    {
        writer.WriteStartObject();
        switch (payload)
        {
            case PresentationArpPayload arp:
                writer.WriteString("kind", "arp"); writer.WriteString("operation", arp.Operation); writer.WriteString("sender_mac", arp.SenderMac); writer.WriteString("sender_ip", arp.SenderIp); writer.WriteString("target_mac", arp.TargetMac); writer.WriteString("target_ip", arp.TargetIp); break;
            case PresentationIpv4EthernetPayload ipv4:
                writer.WriteString("kind", "ipv4"); writer.WriteString("src_ip", ipv4.SourceIp); writer.WriteString("dst_ip", ipv4.DestinationIp); writer.WriteString("ttl", ipv4.Ttl); writer.WritePropertyName("payload"); WriteIpv4Payload(writer, ipv4.Payload); break;
            default: throw new InvalidOperationException($"Unsupported Ethernet payload {payload.GetType().Name}.");
        }
        writer.WriteEndObject();
    }

    private static void WriteIpv4Payload(Utf8JsonWriter writer, PresentationIpv4Payload payload)
    {
        writer.WriteStartObject(); WriteIpv4PayloadFields(writer, payload); writer.WriteEndObject();
    }

    private static void WriteIpv4PayloadFields(Utf8JsonWriter writer, PresentationIpv4Payload payload)
    {
        if (payload is PresentationIcmpPayload icmp)
        {
            writer.WriteString("kind", "icmp"); writer.WriteString("icmp_kind", icmp.IcmpKind); writer.WriteString("identifier", icmp.Identifier); writer.WriteString("sequence", icmp.Sequence);
            return;
        }
        throw new InvalidOperationException($"Unsupported IPv4 payload {payload.GetType().Name}.");
    }

    private static void WriteTableDelta(Utf8JsonWriter writer, PresentationTableDelta delta)
    {
        writer.WriteStartObject();
        switch (delta)
        {
            case PresentationArpLearned learned:
                writer.WriteString("kind", "arp_learned"); writer.WriteString("ip", learned.Ip); writer.WriteString("mac", learned.Mac); writer.WriteString("expires_at_ns", learned.ExpiresAtNs); break;
            case PresentationArpEntryExpired expired:
                writer.WriteString("kind", "arp_entry_expired"); writer.WriteString("ip", expired.Ip); break;
            case PresentationMacLearned learned:
                writer.WriteString("kind", "mac_learned"); writer.WriteString("mac", learned.Mac); writer.WriteString("port_interface_id", learned.PortInterfaceId); writer.WriteString("expires_at_ns", learned.ExpiresAtNs); break;
            case PresentationMacEntryExpired expired:
                writer.WriteString("kind", "mac_entry_expired"); writer.WriteString("mac", expired.Mac); break;
            default: throw new InvalidOperationException($"Unsupported table delta {delta.GetType().Name}.");
        }
        writer.WriteEndObject();
    }

    private static void WritePing(Utf8JsonWriter writer, PresentationPingResult ping)
    {
        writer.WriteStartObject(); writer.WriteString("target_ip", ping.TargetIp); writer.WriteString("identifier", ping.Identifier); writer.WriteString("sequence", ping.Sequence); WriteOptionalString(writer, "rtt_ns", ping.RttNs); WriteOptionalString(writer, "failure_reason", ping.FailureReason); writer.WriteEndObject();
    }
}

public sealed record PresentationTopology(IReadOnlyList<PresentationDevice> Devices, IReadOnlyList<PresentationInterface> Interfaces, IReadOnlyList<PresentationLink> Links);
public sealed record PresentationDevice(string Id, string Kind);
public sealed record PresentationInterface(string Id, string DeviceId, string Name);
public sealed record PresentationLink(string Id, string AInterfaceId, string BInterfaceId);
public sealed record PresentationExecution(string FinalVirtualTimeNs, string Outcome);
public sealed record TraceRetention(string TotalEmittedEver, string DroppedByCapacity, string? OldestAvailableTraceSeq, string? NewestAvailableTraceSeq);
public sealed record PresentationEvent(string TraceSeq, string AtNs, string Kind, PresentationActor Actor, PresentationEndpoint? Ingress, PresentationEndpoint? Egress, PresentationEndpoint? Destination, string? LinkId, IReadOnlyList<string> Protocols, PresentationPduObservation? Pdu, PresentationTableDelta? TableDelta, string? DropReason, PresentationPingResult? Ping);
public sealed record PresentationActor(string DeviceId, string? InterfaceId);
public sealed record PresentationEndpoint(string DeviceId, string InterfaceId);
public sealed record PresentationPduObservation(string Disposition, PresentationPdu Pdu);
public abstract record PresentationPdu;
public sealed record PresentationEthernetPdu(string SourceMac, string DestinationMac, PresentationEthernetPayload Payload) : PresentationPdu;
public sealed record PresentationIpv4Pdu(string SourceIp, string DestinationIp, string Ttl, PresentationIpv4Payload Payload) : PresentationPdu;
public abstract record PresentationEthernetPayload;
public sealed record PresentationArpPayload(string Operation, string SenderMac, string SenderIp, string TargetMac, string TargetIp) : PresentationEthernetPayload;
public sealed record PresentationIpv4EthernetPayload(string SourceIp, string DestinationIp, string Ttl, PresentationIpv4Payload Payload) : PresentationEthernetPayload;
public abstract record PresentationIpv4Payload;
public sealed record PresentationIcmpPayload(string IcmpKind, string Identifier, string Sequence) : PresentationIpv4Payload;
public abstract record PresentationTableDelta;
public sealed record PresentationArpLearned(string Ip, string Mac, string ExpiresAtNs) : PresentationTableDelta;
public sealed record PresentationArpEntryExpired(string Ip) : PresentationTableDelta;
public sealed record PresentationMacLearned(string Mac, string PortInterfaceId, string ExpiresAtNs) : PresentationTableDelta;
public sealed record PresentationMacEntryExpired(string Mac) : PresentationTableDelta;
public sealed record PresentationPingResult(string TargetIp, string Identifier, string Sequence, string? RttNs, string? FailureReason);

public enum PresentationContractErrorCode
{
    UnsupportedSchema,
    BlankField,
    InvalidDecimal,
    DuplicateDeviceId,
    DuplicateInterfaceId,
    DuplicateLinkId,
    UnknownInterfaceDevice,
    UnknownLinkInterface,
    SelfReferentialLink,
    UnknownEventInterface,
    UnknownEventLink,
    TraceRetentionMismatch,
    TraceRetentionBoundsMismatch,
    NonContiguousTraceSequence,
    NonMonotonicVirtualTime
}

public sealed record PresentationContractError(PresentationContractErrorCode Code, string Field, string? DeviceId = null, string? InterfaceId = null, string? LinkId = null, string? TraceSeq = null);
