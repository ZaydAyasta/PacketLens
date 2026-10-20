import { describe, expect, it } from 'vitest'
import { PresentationContractError, parsePresentationSnapshot } from './presentationContract'

const valid = {
  schema_version: 'presentation/v1', scenario_id: 'E1-S2', run_label: 'cold',
  topology: { devices: [{ id: 'pc1', kind: 'host' }, { id: 'sw1', kind: 'l2_switch' }], interfaces: [{ id: 'pc1:eth0', device_id: 'pc1', name: 'eth0' }, { id: 'sw1:port1', device_id: 'sw1', name: 'port1' }], links: [{ id: 'link1', a_interface_id: 'pc1:eth0', b_interface_id: 'sw1:port1' }] },
  execution: { final_virtual_time_ns: '0', outcome: 'Idle' },
  trace_retention: { total_emitted_ever: '0', dropped_by_capacity: '0', oldest_available_trace_seq: null, newest_available_trace_seq: null },
  events: [],
}

describe('parsePresentationSnapshot', () => {
  it('rejects unsupported versions and dangling link interface references', () => {
    expect(() => parsePresentationSnapshot({ ...valid, schema_version: 'presentation/v2' })).toThrow(PresentationContractError)
    expect(() => parsePresentationSnapshot({ ...valid, topology: { ...valid.topology, links: [{ ...valid.topology.links[0], b_interface_id: 'missing:eth0' }] } })).toThrow(/unknown interface/i)
  })

  it('preserves decimal strings instead of converting them to JavaScript numbers', () => {
    const parsed = parsePresentationSnapshot({ ...valid, execution: { final_virtual_time_ns: '9007199254740993', outcome: 'Idle' } })

    expect(parsed.execution.final_virtual_time_ns).toBe('9007199254740993')
  })
})
