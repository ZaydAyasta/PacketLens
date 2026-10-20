import { describe, expect, it } from 'vitest'
import { createPresentationState } from './presentationState'

const snapshot = {
  schema_version: 'presentation/v1',
  scenario_id: 'E1-S2',
  run_label: 'cold',
  topology: { devices: [], interfaces: [], links: [] },
  execution: { final_virtual_time_ns: '8700', outcome: 'Quiescent' },
  trace_retention: {
    total_emitted_ever: '2',
    dropped_by_capacity: '0',
    oldest_available_trace_seq: '1',
    newest_available_trace_seq: '2',
  },
  events: [
    { trace_seq: '1', at_ns: '0', kind: 'arp_packet_queued', actor: { device_id: 'pc1', interface_id: null }, ingress: null, egress: null, destination: null, link_id: null, protocols: ['arp'], pdu: null, table_delta: null, drop_reason: null, ping: null },
    { trace_seq: '2', at_ns: '1000', kind: 'packet_dropped', actor: { device_id: 'pc3', interface_id: 'pc3:eth0' }, ingress: null, egress: null, destination: null, link_id: null, protocols: ['arp'], pdu: null, table_delta: null, drop_reason: 'arp_target_ip_mismatch', ping: null },
  ],
} as const

describe('createPresentationState', () => {
  it('filters without mutating the engine event order and selects by trace_seq', () => {
    const state = createPresentationState(snapshot)

    const filtered = state.filteredEvents({ deviceId: 'pc3' })
    state.select('2')

    expect(filtered.map((event) => event.trace_seq)).toEqual(['2'])
    expect(state.selected.value?.at_ns).toBe('1000')
    expect(snapshot.events.map((event) => event.trace_seq)).toEqual(['1', '2'])
  })

  it('navigates the visual cursor without changing the source snapshot', () => {
    const state = createPresentationState(snapshot)

    state.select('2')
    state.previous()

    expect(state.selected.value?.trace_seq).toBe('1')
    expect(state.snapshot.events).toHaveLength(2)
  })
})
