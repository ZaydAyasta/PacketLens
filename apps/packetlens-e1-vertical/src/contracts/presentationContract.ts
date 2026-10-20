import { PRESENTATION_SCHEMA_VERSION, type PresentationEndpoint, type PresentationEvent, type PresentationPdu, type SimulationPresentationSnapshot } from './presentation'

const outcomes = new Set(['Idle', 'Quiescent', 'TargetTimeReached', 'BudgetExhausted', 'HaltedBySafetyGuard'])
const eventKinds = new Set(['frame_transmitted', 'frame_delivered', 'packet_dropped', 'arp_packet_queued', 'table_updated', 'ping_completed', 'ping_failed'])
const decimal = /^(0|[1-9][0-9]*)$/

export class PresentationContractError extends Error {
  constructor(message: string) { super(message); this.name = 'PresentationContractError' }
}

export function parsePresentationSnapshot(input: unknown): SimulationPresentationSnapshot {
  const snapshot = object(input, 'snapshot')
  equal(snapshot.schema_version, PRESENTATION_SCHEMA_VERSION, 'schema_version')
  text(snapshot.scenario_id, 'scenario_id'); text(snapshot.run_label, 'run_label')
  const topology = object(snapshot.topology, 'topology')
  const devices = array(topology.devices, 'topology.devices')
  const interfaces = array(topology.interfaces, 'topology.interfaces')
  const links = array(topology.links, 'topology.links')
  const deviceIds = unique(devices.map((value) => text(object(value, 'topology.devices').id, 'topology.devices[].id')), 'device')
  const interfaceIds = unique(interfaces.map((value) => {
    const networkInterface = object(value, 'topology.interfaces')
    const id = text(networkInterface.id, 'topology.interfaces[].id')
    const deviceId = text(networkInterface.device_id, 'topology.interfaces[].device_id')
    text(networkInterface.name, 'topology.interfaces[].name')
    if (!deviceIds.has(deviceId)) fail(`Interface ${id} references unknown device ${deviceId}.`)
    return id
  }), 'interface')
  unique(links.map((value) => {
    const link = object(value, 'topology.links')
    const id = text(link.id, 'topology.links[].id')
    const a = text(link.a_interface_id, 'topology.links[].a_interface_id')
    const b = text(link.b_interface_id, 'topology.links[].b_interface_id')
    if (!interfaceIds.has(a) || !interfaceIds.has(b)) fail(`Link ${id} references an unknown interface.`)
    if (a === b) fail(`Link ${id} cannot reference the same interface twice.`)
    return id
  }), 'link')

  const execution = object(snapshot.execution, 'execution')
  decimalString(execution.final_virtual_time_ns, 'execution.final_virtual_time_ns')
  if (!hasString(outcomes, execution.outcome)) fail('execution.outcome is not a supported discriminator.')
  const retention = object(snapshot.trace_retention, 'trace_retention')
  const total = decimalValue(retention.total_emitted_ever, 'trace_retention.total_emitted_ever')
  const dropped = decimalValue(retention.dropped_by_capacity, 'trace_retention.dropped_by_capacity')
  if (dropped > total) fail('trace_retention.dropped_by_capacity exceeds total_emitted_ever.')
  const events = array(snapshot.events, 'events')
  if (total - dropped !== BigInt(events.length)) fail('trace_retention does not match the available event count.')
  const oldest = optionalDecimal(retention.oldest_available_trace_seq, 'trace_retention.oldest_available_trace_seq')
  const newest = optionalDecimal(retention.newest_available_trace_seq, 'trace_retention.newest_available_trace_seq')
  let lastSequence: bigint | null = null
  let lastTime: bigint | null = null
  for (const value of events) {
    validateEvent(value, interfaceIds, lastSequence, lastTime)
    const event = object(value, 'event')
    lastSequence = decimalValue(event.trace_seq, 'events[].trace_seq')
    lastTime = decimalValue(event.at_ns, 'events[].at_ns')
  }
  if (events.length === 0 ? oldest !== null || newest !== null : oldest !== object(events[0], 'event').trace_seq || newest !== object(events.at(-1), 'event').trace_seq) {
    fail('trace_retention bounds do not match retained events.')
  }
  return input as SimulationPresentationSnapshot
}

function validateEvent(value: unknown, interfaceIds: Set<string>, previousSequence: bigint | null, previousTime: bigint | null): void {
  const event = object(value, 'event')
  const sequence = decimalValue(event.trace_seq, 'events[].trace_seq')
  const time = decimalValue(event.at_ns, 'events[].at_ns')
  if (previousSequence !== null && sequence !== previousSequence + 1n) fail('events must have contiguous trace_seq values.')
  if (previousTime !== null && time < previousTime) fail('events must be in nondecreasing virtual-time order.')
  if (!hasString(eventKinds, event.kind)) fail('events[].kind is not a supported discriminator.')
  const actor = object(event.actor, 'events[].actor')
  text(actor.device_id, 'events[].actor.device_id')
  optionalInterface(actor.interface_id, interfaceIds, 'events[].actor.interface_id')
  optionalEndpoint(event.ingress, interfaceIds); optionalEndpoint(event.egress, interfaceIds); optionalEndpoint(event.destination, interfaceIds)
  if (!Array.isArray(event.protocols) || event.protocols.some((protocol) => typeof protocol !== 'string' || !protocol)) fail('events[].protocols must contain nonblank strings.')
  if (event.pdu !== null) validatePduObservation(event.pdu)
  if (event.table_delta !== null) validateTableDelta(event.table_delta)
}

function validatePduObservation(value: unknown): void {
  const observation = object(value, 'events[].pdu')
  if (!hasString(['inbound', 'outbound', 'queued', 'dropped'], observation.disposition)) fail('events[].pdu.disposition is invalid.')
  validatePdu(observation.pdu)
}

function validatePdu(value: unknown): void {
  const pdu = object(value, 'pdu')
  if (pdu.kind === 'ethernet') {
    text(pdu.src_mac, 'pdu.src_mac'); text(pdu.dst_mac, 'pdu.dst_mac'); validatePduPayload(pdu.payload); return
  }
  if (pdu.kind === 'ipv4') {
    text(pdu.src_ip, 'pdu.src_ip'); text(pdu.dst_ip, 'pdu.dst_ip'); decimalString(pdu.ttl, 'pdu.ttl'); validateIcmpPayload(pdu.payload); return
  }
  fail('pdu.kind is invalid.')
}

function validatePduPayload(value: unknown): void {
  const payload = object(value, 'ethernet payload')
  if (payload.kind === 'arp') {
    if (!hasString(['request', 'reply'], payload.operation)) fail('ARP operation is invalid.')
    for (const field of ['sender_mac', 'sender_ip', 'target_mac', 'target_ip']) text(payload[field], `ARP.${field}`)
    return
  }
  if (payload.kind === 'ipv4') { validatePdu(payload); return }
  fail('Ethernet payload kind is invalid.')
}

function validateIcmpPayload(value: unknown): void {
  const payload = object(value, 'IPv4 payload')
  if (payload.kind !== 'icmp') fail('IPv4 payload kind is invalid.')
  if (!hasString(['echo_request', 'echo_reply', 'destination_host_unreachable', 'time_exceeded'], payload.icmp_kind)) fail('ICMP kind is invalid.')
  decimalString(payload.identifier, 'icmp.identifier'); decimalString(payload.sequence, 'icmp.sequence')
}

function validateTableDelta(value: unknown): void {
  const delta = object(value, 'events[].table_delta')
  if (!hasString(['arp_learned', 'arp_entry_expired', 'mac_learned', 'mac_entry_expired'], delta.kind)) fail('table_delta.kind is invalid.')
  if (delta.kind === 'arp_learned') { text(delta.ip, 'table_delta.ip'); text(delta.mac, 'table_delta.mac'); decimalString(delta.expires_at_ns, 'table_delta.expires_at_ns') }
  if (delta.kind === 'arp_entry_expired') text(delta.ip, 'table_delta.ip')
  if (delta.kind === 'mac_learned') { text(delta.mac, 'table_delta.mac'); text(delta.port_interface_id, 'table_delta.port_interface_id'); decimalString(delta.expires_at_ns, 'table_delta.expires_at_ns') }
  if (delta.kind === 'mac_entry_expired') text(delta.mac, 'table_delta.mac')
}

function optionalEndpoint(value: unknown, interfaces: Set<string>): void {
  if (value === null) return
  const endpoint = object(value, 'event endpoint')
  text(endpoint.device_id, 'event endpoint.device_id')
  optionalInterface(endpoint.interface_id, interfaces, 'event endpoint.interface_id')
}

function optionalInterface(value: unknown, interfaces: Set<string>, field: string): void {
  if (value === null) return
  const interfaceId = text(value, field)
  if (!interfaces.has(interfaceId)) fail(`${field} references an unknown interface.`)
}

function object(value: unknown, field: string): Record<string, unknown> { if (typeof value !== 'object' || value === null || Array.isArray(value)) fail(`${field} must be an object.`); return value as Record<string, unknown> }
function array(value: unknown, field: string): readonly unknown[] { if (!Array.isArray(value)) fail(`${field} must be an array.`); return value }
function text(value: unknown, field: string): string { if (typeof value !== 'string' || !value.trim()) fail(`${field} must be a nonblank string.`); return value }
function decimalString(value: unknown, field: string): string { const stringValue = text(value, field); if (!decimal.test(stringValue)) fail(`${field} must be a decimal string.`); return stringValue }
function decimalValue(value: unknown, field: string): bigint { return BigInt(decimalString(value, field)) }
function optionalDecimal(value: unknown, field: string): string | null { return value === null ? null : decimalString(value, field) }
function unique(values: readonly string[], category: string): Set<string> { const result = new Set(values); if (result.size !== values.length) fail(`Duplicate ${category} identifier.`); return result }
function equal(value: unknown, expected: string, field: string): void { if (value !== expected) fail(`${field} must equal ${expected}.`) }
function hasString(values: Iterable<string>, candidate: unknown): candidate is string {
  return typeof candidate === 'string' && Array.from(values).includes(candidate)
}
function fail(message: string): never { throw new PresentationContractError(message) }
