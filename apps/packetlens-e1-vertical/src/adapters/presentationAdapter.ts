import type {
  PresentationEthernetPdu,
  PresentationEvent,
  PresentationIpv4Pdu,
  PresentationPduObservation,
  PresentationTableDelta,
  SimulationPresentationSnapshot,
} from '../contracts/presentation'
import { PDU_FIELD_NOTES, explainEvent } from '../catalog/pedagogicalCatalog'
import type {
  DeviceTablesViewModel,
  InspectorEventViewModel,
  PduFieldViewModel,
  PduHeaderBlockViewModel,
  PduStructuredViewModel,
  ProtocolType,
  TableDeltaViewModel,
  TableEntryViewModel,
  TopologyViewModel,
} from '../types/inspector'

export interface AdaptedSimulationData {
  readonly scenarioId: string
  readonly runLabel: string
  readonly finalVirtualTimeFormatted: string
  readonly isQuiescent: boolean
  readonly totalEmitted: bigint
  readonly droppedByCapacity: bigint
  readonly isTruncated: boolean
  readonly isHistoricalStateIncomplete: boolean
  readonly events: readonly InspectorEventViewModel[]
  readonly topology: TopologyViewModel
}

interface MutableTableState {
  arp: Map<string, TableEntryViewModel>
  cam: Map<string, TableEntryViewModel>
}

export function formatVirtualTimeBigInt(value: string): string {
  const ns = BigInt(value)
  if (ns === 0n) return '0 ns'
  if (ns < 1000n) return `${ns} ns`
  if (ns < 1_000_000n) return `${ns / 1000n}.${(ns % 1000n).toString().padStart(3, '0')} µs`
  if (ns < 1_000_000_000n) return `${ns / 1_000_000n}.${((ns % 1_000_000n) / 1000n).toString().padStart(3, '0')} ms`
  return `${ns / 1_000_000_000n}.${((ns % 1_000_000_000n) / 1_000_000n).toString().padStart(3, '0')} s`
}

export function adaptPresentationSnapshot(snapshot: SimulationPresentationSnapshot): AdaptedSimulationData {
  const droppedByCapacity = BigInt(snapshot.trace_retention.dropped_by_capacity)
  const isHistoricalStateIncomplete = droppedByCapacity > 0n
  const topology = adaptTopology(snapshot)
  const tableState = initialTableState(snapshot)
  const events: InspectorEventViewModel[] = []
  let previous: PresentationEvent | null = null

  for (const event of snapshot.events) {
    applyDelta(tableState, event.actor.device_id, event.table_delta)
    const pdu = adaptPdu(event.pdu)
    const explanation = explainEvent(event, previous)
    events.push({
      traceSeq: event.trace_seq,
      atNs: event.at_ns,
      atNsBigInt: BigInt(event.at_ns),
      timestampFormatted: formatVirtualTimeBigInt(event.at_ns),
      kind: event.kind,
      deviceId: event.actor.device_id,
      interfaceId: event.actor.interface_id,
      ingress: event.ingress,
      egress: event.egress,
      destination: event.destination,
      linkId: event.link_id,
      protocols: event.protocols,
      protocol: primaryProtocol(event),
      summary: explanation.summary,
      pedagogicalRationale: explanation.rationale,
      pduIn: event.pdu?.disposition === 'inbound' || event.pdu?.disposition === 'dropped' ? pdu : null,
      pduOut: event.pdu?.disposition === 'outbound' || event.pdu?.disposition === 'queued' ? pdu : null,
      tableDelta: adaptDelta(event.table_delta),
      dropReason: event.drop_reason,
      tablesSnapshot: snapshotTables(tableState, isHistoricalStateIncomplete, event.actor.device_id, event.table_delta),
    })
    previous = event
  }

  return {
    scenarioId: snapshot.scenario_id,
    runLabel: snapshot.run_label,
    finalVirtualTimeFormatted: formatVirtualTimeBigInt(snapshot.execution.final_virtual_time_ns),
    isQuiescent: snapshot.execution.outcome === 'Quiescent',
    totalEmitted: BigInt(snapshot.trace_retention.total_emitted_ever),
    droppedByCapacity,
    isTruncated: isHistoricalStateIncomplete,
    isHistoricalStateIncomplete,
    events,
    topology,
  }
}

function adaptTopology(snapshot: SimulationPresentationSnapshot): TopologyViewModel {
  const positions: Readonly<Record<string, readonly [number, number]>> = {
    pc1: [15, 50],
    sw1: [50, 50],
    pc2: [84, 28],
    pc3: [84, 72],
  }
  return {
    nodes: snapshot.topology.devices.map((device, index) => {
      const fallback: readonly [number, number] = [20 + index * 20, 50]
      const [x, y] = positions[device.id] ?? fallback
      return { id: device.id, kind: device.kind, label: device.id.toUpperCase(), x, y }
    }),
    interfaces: snapshot.topology.interfaces.map((networkInterface) => ({
      id: networkInterface.id,
      deviceId: networkInterface.device_id,
      name: networkInterface.name,
    })),
    links: snapshot.topology.links.map((link) => ({
      id: link.id,
      aInterfaceId: link.a_interface_id,
      bInterfaceId: link.b_interface_id,
    })),
  }
}

function initialTableState(snapshot: SimulationPresentationSnapshot): Map<string, MutableTableState> {
  return new Map(snapshot.topology.devices.map((device) => [device.id, { arp: new Map(), cam: new Map() }]))
}

function applyDelta(state: Map<string, MutableTableState>, deviceId: string, delta: PresentationTableDelta | null): void {
  if (delta === null) return
  const device = state.get(deviceId)
  if (device === undefined) return
  if (delta.kind === 'arp_learned') {
    device.arp.set(delta.ip, entry(delta.ip, delta.mac, delta.expires_at_ns, true))
    return
  }
  if (delta.kind === 'arp_entry_expired') {
    device.arp.delete(delta.ip)
    return
  }
  if (delta.kind === 'mac_learned') {
    device.cam.set(delta.mac, entry(delta.mac, delta.port_interface_id, delta.expires_at_ns, true))
    return
  }
  device.cam.delete(delta.mac)
}

function entry(key: string, value: string, expiresAtNs: string, isNewInCurrentEvent: boolean): TableEntryViewModel {
  return { key, value, expiresAtNs, isNewInCurrentEvent }
}

function snapshotTables(
  state: ReadonlyMap<string, MutableTableState>,
  incomplete: boolean,
  currentActorId: string,
  currentDelta: PresentationTableDelta | null,
): Readonly<Record<string, DeviceTablesViewModel>> {
  const result: Record<string, DeviceTablesViewModel> = {}
  for (const [deviceId, tables] of state.entries()) {
    result[deviceId] = {
      deviceId,
      arp: Array.from(tables.arp.values(), (tableEntry) => ({
        ...tableEntry,
        isNewInCurrentEvent: deviceId === currentActorId && currentDelta?.kind === 'arp_learned' && currentDelta.ip === tableEntry.key,
      })),
      cam: Array.from(tables.cam.values(), (tableEntry) => ({
        ...tableEntry,
        isNewInCurrentEvent: deviceId === currentActorId && currentDelta?.kind === 'mac_learned' && currentDelta.mac === tableEntry.key,
      })),
      incomplete,
    }
  }
  return result
}

function adaptDelta(delta: PresentationTableDelta | null): TableDeltaViewModel | null {
  if (delta === null) return null
  if (delta.kind === 'arp_learned') {
    return {
      kind: delta.kind,
      tableKind: 'arp',
      operation: 'learned',
      key: delta.ip,
      value: delta.mac,
      expiresAtNs: delta.expires_at_ns,
      explanation: `Hecho del motor: aprendizaje ARP ${delta.ip} -> ${delta.mac}.`,
    }
  }
  if (delta.kind === 'arp_entry_expired') {
    return {
      kind: delta.kind,
      tableKind: 'arp',
      operation: 'expired',
      key: delta.ip,
      value: null,
      expiresAtNs: null,
      explanation: `Hecho del motor: expiración ARP de ${delta.ip}.`,
    }
  }
  if (delta.kind === 'mac_learned') {
    return {
      kind: delta.kind,
      tableKind: 'cam',
      operation: 'learned',
      key: delta.mac,
      value: delta.port_interface_id,
      expiresAtNs: delta.expires_at_ns,
      explanation: `Hecho del motor: aprendizaje CAM ${delta.mac} -> ${delta.port_interface_id}.`,
    }
  }
  return {
    kind: delta.kind,
    tableKind: 'cam',
    operation: 'expired',
    key: delta.mac,
    value: null,
    expiresAtNs: null,
    explanation: `Hecho del motor: expiración CAM de ${delta.mac}.`,
  }
}

function primaryProtocol(event: PresentationEvent): ProtocolType {
  if (event.protocols.includes('icmp')) return 'ICMP'
  if (event.protocols.includes('arp')) return 'ARP'
  if (event.protocols.includes('ipv4')) return 'IPV4'
  if (event.protocols.includes('ethernet')) return 'ETHERNET'
  return 'NONE'
}

function adaptPdu(observation: PresentationPduObservation | null): PduStructuredViewModel | null {
  if (observation === null) return null
  const blocks: PduHeaderBlockViewModel[] = []
  if (observation.pdu.kind === 'ethernet') adaptEthernet(observation.pdu, blocks)
  else adaptIpv4(observation.pdu, blocks)
  return {
    disposition: observation.disposition,
    summary: `PDU ${observation.disposition}: ${blocks.map((block) => block.protocol).join(' / ')}`,
    blocks,
  }
}

function adaptEthernet(ethernet: PresentationEthernetPdu, blocks: PduHeaderBlockViewModel[]): void {
  const carriesArp = ethernet.payload.kind === 'arp'
  blocks.push({
    title: 'Ethernet II',
    protocol: 'ETHERNET',
    fields: [
      field('Destination MAC', ethernet.dst_mac, 'engine_fact', PDU_FIELD_NOTES.ethernet_dst),
      field('Source MAC', ethernet.src_mac, 'engine_fact', PDU_FIELD_NOTES.ethernet_src),
      field('Payload type', carriesArp ? 'ARP' : 'IPv4', 'didactic_context', PDU_FIELD_NOTES.ethernet_type),
      field('FCS / CRC32', 'No modelado en E1', 'unmodeled', PDU_FIELD_NOTES.ethernet_fcs),
    ],
  })
  if (carriesArp) {
    const arp = ethernet.payload
    blocks.push({
      title: 'ARP',
      protocol: 'ARP',
      fields: [
        field('Hardware type', 'Ethernet', 'didactic_context', PDU_FIELD_NOTES.arp_context),
        field('Protocol type', 'IPv4', 'didactic_context', PDU_FIELD_NOTES.arp_context),
        field('Operation', arp.operation, 'engine_fact', PDU_FIELD_NOTES.arp_operation),
        field('Sender MAC', arp.sender_mac, 'engine_fact', PDU_FIELD_NOTES.arp_sender),
        field('Sender IP', arp.sender_ip, 'engine_fact', PDU_FIELD_NOTES.arp_sender),
        field('Target MAC', arp.target_mac, 'engine_fact', PDU_FIELD_NOTES.arp_target),
        field('Target IP', arp.target_ip, 'engine_fact', PDU_FIELD_NOTES.arp_target),
      ],
    })
    return
  }
  adaptIpv4(ethernet.payload, blocks)
}

function adaptIpv4(ipv4: PresentationIpv4Pdu, blocks: PduHeaderBlockViewModel[]): void {
  blocks.push({
    title: 'IPv4',
    protocol: 'IPV4',
    fields: [
      field('Version / IHL', 'IPv4 / no instrumentado', 'didactic_context', PDU_FIELD_NOTES.ipv4_context),
      field('Source IP', ipv4.src_ip, 'engine_fact', PDU_FIELD_NOTES.ipv4_addresses),
      field('Destination IP', ipv4.dst_ip, 'engine_fact', PDU_FIELD_NOTES.ipv4_addresses),
      field('TTL', ipv4.ttl, 'engine_fact', PDU_FIELD_NOTES.ipv4_ttl),
      field('Header checksum', 'No modelado en E1', 'unmodeled', PDU_FIELD_NOTES.ipv4_context),
    ],
  })
  blocks.push({
    title: 'ICMP',
    protocol: 'ICMP',
    fields: [
      field('Kind', ipv4.payload.icmp_kind, 'engine_fact', PDU_FIELD_NOTES.icmp_kind),
      field('Identifier', ipv4.payload.identifier, 'engine_fact', PDU_FIELD_NOTES.icmp_sequence),
      field('Sequence', ipv4.payload.sequence, 'engine_fact', PDU_FIELD_NOTES.icmp_sequence),
      field('Code', 'No modelado en E1', 'unmodeled', PDU_FIELD_NOTES.icmp_context),
      field('Checksum', 'No modelado en E1', 'unmodeled', PDU_FIELD_NOTES.icmp_context),
    ],
  })
}

function field(name: string, value: string, fidelity: PduFieldViewModel['fidelity'], educationalNote: string): PduFieldViewModel {
  return { name, value, fidelity, educationalNote }
}

