export const PRESENTATION_SCHEMA_VERSION = 'presentation/v1' as const

export type DecimalString = string
export type DeviceKind = 'host' | 'l2_switch'
export type ExecutionOutcome = 'Idle' | 'Quiescent' | 'TargetTimeReached' | 'BudgetExhausted' | 'HaltedBySafetyGuard'
export type EventKind = 'frame_transmitted' | 'frame_delivered' | 'packet_dropped' | 'arp_packet_queued' | 'table_updated' | 'ping_completed' | 'ping_failed'
export type PduDisposition = 'inbound' | 'outbound' | 'queued' | 'dropped'

export interface SimulationPresentationSnapshot {
  readonly schema_version: typeof PRESENTATION_SCHEMA_VERSION
  readonly scenario_id: string
  readonly run_label: string
  readonly topology: PresentationTopology
  readonly execution: PresentationExecution
  readonly trace_retention: TraceRetention
  readonly events: readonly PresentationEvent[]
}

export interface PresentationTopology {
  readonly devices: readonly PresentationDevice[]
  readonly interfaces: readonly PresentationInterface[]
  readonly links: readonly PresentationLink[]
}

export interface PresentationDevice { readonly id: string; readonly kind: DeviceKind }
export interface PresentationInterface { readonly id: string; readonly device_id: string; readonly name: string }
export interface PresentationLink { readonly id: string; readonly a_interface_id: string; readonly b_interface_id: string }
export interface PresentationExecution { readonly final_virtual_time_ns: DecimalString; readonly outcome: ExecutionOutcome }
export interface TraceRetention { readonly total_emitted_ever: DecimalString; readonly dropped_by_capacity: DecimalString; readonly oldest_available_trace_seq: DecimalString | null; readonly newest_available_trace_seq: DecimalString | null }
export interface PresentationActor { readonly device_id: string; readonly interface_id: string | null }
export interface PresentationEndpoint { readonly device_id: string; readonly interface_id: string }
export interface PresentationPduObservation { readonly disposition: PduDisposition; readonly pdu: PresentationPdu }

export type PresentationPdu = PresentationEthernetPdu | PresentationIpv4Pdu
export interface PresentationEthernetPdu { readonly kind: 'ethernet'; readonly src_mac: string; readonly dst_mac: string; readonly payload: PresentationArpPayload | PresentationIpv4Pdu }
export interface PresentationIpv4Pdu { readonly kind: 'ipv4'; readonly src_ip: string; readonly dst_ip: string; readonly ttl: DecimalString; readonly payload: PresentationIcmpPayload }
export interface PresentationArpPayload { readonly kind: 'arp'; readonly operation: 'request' | 'reply'; readonly sender_mac: string; readonly sender_ip: string; readonly target_mac: string; readonly target_ip: string }
export interface PresentationIcmpPayload { readonly kind: 'icmp'; readonly icmp_kind: 'echo_request' | 'echo_reply' | 'destination_host_unreachable' | 'time_exceeded'; readonly identifier: DecimalString; readonly sequence: DecimalString }

export type PresentationTableDelta = PresentationArpLearned | PresentationArpEntryExpired | PresentationMacLearned | PresentationMacEntryExpired
export interface PresentationArpLearned { readonly kind: 'arp_learned'; readonly ip: string; readonly mac: string; readonly expires_at_ns: DecimalString }
export interface PresentationArpEntryExpired { readonly kind: 'arp_entry_expired'; readonly ip: string }
export interface PresentationMacLearned { readonly kind: 'mac_learned'; readonly mac: string; readonly port_interface_id: string; readonly expires_at_ns: DecimalString }
export interface PresentationMacEntryExpired { readonly kind: 'mac_entry_expired'; readonly mac: string }
export interface PresentationPingResult { readonly target_ip: string; readonly identifier: DecimalString; readonly sequence: DecimalString; readonly rtt_ns: DecimalString | null; readonly failure_reason: 'arp_timeout' | null }

export interface PresentationEvent {
  readonly trace_seq: DecimalString
  readonly at_ns: DecimalString
  readonly kind: EventKind
  readonly actor: PresentationActor
  readonly ingress: PresentationEndpoint | null
  readonly egress: PresentationEndpoint | null
  readonly destination: PresentationEndpoint | null
  readonly link_id: string | null
  readonly protocols: readonly string[]
  readonly pdu: PresentationPduObservation | null
  readonly table_delta: PresentationTableDelta | null
  readonly drop_reason: string | null
  readonly ping: PresentationPingResult | null
}
