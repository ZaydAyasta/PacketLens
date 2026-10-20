import type {
  EventKind,
  PduDisposition,
  PresentationEndpoint,
} from '../contracts/presentation'

export type ProtocolType = 'ARP' | 'ICMP' | 'ETHERNET' | 'IPV4' | 'NONE'
export type DataFidelityKind = 'engine_fact' | 'didactic_context' | 'unmodeled'
export type SnapshotSource = 'saved_engine_fixture' | 'live_engine' | 'synthetic_fixture'

export interface PduFieldViewModel {
  readonly name: string
  readonly value: string
  readonly fidelity: DataFidelityKind
  readonly educationalNote: string
}

export interface PduHeaderBlockViewModel {
  readonly title: string
  readonly protocol: Exclude<ProtocolType, 'NONE'>
  readonly fields: readonly PduFieldViewModel[]
}

export interface PduStructuredViewModel {
  readonly disposition: PduDisposition
  readonly summary: string
  readonly blocks: readonly PduHeaderBlockViewModel[]
}

export interface TableDeltaViewModel {
  readonly kind: 'arp_learned' | 'arp_entry_expired' | 'mac_learned' | 'mac_entry_expired'
  readonly tableKind: 'arp' | 'cam'
  readonly operation: 'learned' | 'expired'
  readonly key: string
  readonly value: string | null
  readonly expiresAtNs: string | null
  readonly explanation: string
}

export interface TableEntryViewModel {
  readonly key: string
  readonly value: string
  readonly expiresAtNs: string | null
  readonly isNewInCurrentEvent: boolean
}

export interface DeviceTablesViewModel {
  readonly deviceId: string
  readonly arp: readonly TableEntryViewModel[]
  readonly cam: readonly TableEntryViewModel[]
  readonly incomplete: boolean
}

export interface InspectorEventViewModel {
  readonly traceSeq: string
  readonly atNs: string
  readonly atNsBigInt: bigint
  readonly timestampFormatted: string
  readonly kind: EventKind
  readonly deviceId: string
  readonly interfaceId: string | null
  readonly ingress: PresentationEndpoint | null
  readonly egress: PresentationEndpoint | null
  readonly destination: PresentationEndpoint | null
  readonly linkId: string | null
  readonly protocols: readonly string[]
  readonly protocol: ProtocolType
  readonly summary: string
  readonly pedagogicalRationale: string
  readonly pduIn: PduStructuredViewModel | null
  readonly pduOut: PduStructuredViewModel | null
  readonly tableDelta: TableDeltaViewModel | null
  readonly dropReason: string | null
  readonly tablesSnapshot: Readonly<Record<string, DeviceTablesViewModel>>
}

export interface TopologyNodeViewModel {
  readonly id: string
  readonly kind: 'host' | 'l2_switch'
  readonly label: string
  readonly x: number
  readonly y: number
}

export interface TopologyLinkViewModel {
  readonly id: string
  readonly aInterfaceId: string
  readonly bInterfaceId: string
}

export interface TopologyViewModel {
  readonly nodes: readonly TopologyNodeViewModel[]
  readonly interfaces: readonly { readonly id: string; readonly deviceId: string; readonly name: string }[]
  readonly links: readonly TopologyLinkViewModel[]
}
