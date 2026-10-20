import type { SimulationPresentationSnapshot } from '../contracts/presentation'

/**
 * Deliberately illustrative UI material. It is not exported by a PacketLens
 * engine and the source selector labels it as synthetic at all times.
 */
export const SYNTHETIC_E1_SNAPSHOT = {
  schema_version: 'presentation/v1',
  scenario_id: 'E1-S2-DEMO',
  run_label: 'synthetic-demonstration',
  topology: {
    devices: [
      { id: 'demo-host', kind: 'host' },
      { id: 'demo-switch', kind: 'l2_switch' },
    ],
    interfaces: [
      { id: 'demo-host:eth0', device_id: 'demo-host', name: 'eth0' },
      { id: 'demo-switch:port1', device_id: 'demo-switch', name: 'port1' },
    ],
    links: [{ id: 'demo-link', a_interface_id: 'demo-host:eth0', b_interface_id: 'demo-switch:port1' }],
  },
  execution: { final_virtual_time_ns: '3000', outcome: 'Quiescent' },
  trace_retention: {
    total_emitted_ever: '3',
    dropped_by_capacity: '0',
    oldest_available_trace_seq: '1',
    newest_available_trace_seq: '3',
  },
  events: [
    {
      trace_seq: '1', at_ns: '0', kind: 'arp_packet_queued',
      actor: { device_id: 'demo-host', interface_id: null }, ingress: null, egress: null, destination: null, link_id: null,
      protocols: ['arp', 'ipv4', 'icmp'],
      pdu: { disposition: 'queued', pdu: { kind: 'ipv4', src_ip: '198.51.100.10', dst_ip: '198.51.100.20', ttl: '64', payload: { kind: 'icmp', icmp_kind: 'echo_request', identifier: '1', sequence: '1' } } },
      table_delta: null, drop_reason: null, ping: null,
    },
    {
      trace_seq: '2', at_ns: '1000', kind: 'table_updated',
      actor: { device_id: 'demo-host', interface_id: null }, ingress: null, egress: null, destination: null, link_id: null,
      protocols: ['arp'], pdu: null,
      table_delta: { kind: 'arp_learned', ip: '198.51.100.20', mac: '02:00:00:00:00:20', expires_at_ns: '60000001000' },
      drop_reason: null, ping: null,
    },
    {
      trace_seq: '3', at_ns: '3000', kind: 'ping_failed',
      actor: { device_id: 'demo-host', interface_id: null }, ingress: null, egress: null, destination: null, link_id: null,
      protocols: ['icmp'], pdu: null, table_delta: null, drop_reason: null,
      ping: { target_ip: '198.51.100.20', identifier: '1', sequence: '1', rtt_ns: null, failure_reason: 'arp_timeout' },
    },
  ],
} as const satisfies SimulationPresentationSnapshot
