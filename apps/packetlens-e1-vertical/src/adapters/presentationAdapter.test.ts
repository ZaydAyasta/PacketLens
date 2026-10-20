import { describe, expect, it } from 'vitest'
import fixture from '../fixtures/e1-s2-cold.presentation.json'
import { parsePresentationSnapshot } from '../contracts/presentationContract'
import {
  adaptPresentationSnapshot,
  formatVirtualTimeBigInt,
} from './presentationAdapter'

describe('presentationAdapter — E1-S2 cold snapshot', () => {
  it('projects the 27 engine events without changing their trace identities', () => {
    const snapshot = parsePresentationSnapshot(fixture)
    const adapted = adaptPresentationSnapshot(snapshot)

    expect(adapted.events).toHaveLength(27)
    expect(adapted.events.map((event) => event.traceSeq)).toEqual(
      Array.from({ length: 27 }, (_, index) => String(index + 1)),
    )
    expect(adapted.finalVirtualTimeFormatted).toBe('8.700 µs')
  })

  it('keeps ARP queueing, CAM learning, and IPv4/ICMP CAM refresh as distinct facts', () => {
    const adapted = adaptPresentationSnapshot(parsePresentationSnapshot(fixture))

    const queued = adapted.events[0]
    expect(queued.kind).toBe('arp_packet_queued')
    expect(queued.pduOut?.blocks.map((block) => block.protocol)).toEqual(['IPV4', 'ICMP'])
    expect(queued.summary).toContain('IPv4/ICMP')
    expect(queued.summary).not.toContain('ARP Request encolado')

    for (const traceSeq of ['4', '19', '24']) {
      const event = adapted.events.find((candidate) => candidate.traceSeq === traceSeq)
      expect(event?.tableDelta?.kind).toBe('mac_learned')
      expect(event?.pedagogicalRationale).toContain('CAM')
    }
    expect(adapted.events.find((candidate) => candidate.traceSeq === '19')?.protocol).toBe('ETHERNET')
    expect(adapted.events.find((candidate) => candidate.traceSeq === '19')?.pedagogicalRationale).toContain('IPv4/ICMP')
  })

  it('formats values past JavaScript safe integer precision using BigInt', () => {
    expect(formatVirtualTimeBigInt('9007199254740992000')).toBe('9007199254.740 s')
  })

  it('keeps PDU fidelity and ARP/CAM state grounded in the emitted fields and deltas', () => {
    const adapted = adaptPresentationSnapshot(parsePresentationSnapshot(fixture))
    const arp = adapted.events.find((event) => event.traceSeq === '2')?.pduOut
    const ethernet = arp?.blocks.find((block) => block.protocol === 'ETHERNET')
    const fcs = ethernet?.fields.find((field) => field.name === 'FCS / CRC32')
    expect(fcs?.fidelity).toBe('unmodeled')
    expect(fcs?.value).toBe('No modelado en E1')

    const ipv4 = adapted.events.find((event) => event.traceSeq === '17')?.pduOut?.blocks.find((block) => block.protocol === 'IPV4')
    expect(ipv4?.fields.find((field) => field.name === 'TTL')?.fidelity).toBe('engine_fact')
    expect(ipv4?.fields.find((field) => field.name === 'Version / IHL')?.fidelity).toBe('didactic_context')

    const camAtFour = adapted.events.find((event) => event.traceSeq === '4')?.tablesSnapshot.sw1.cam
    expect(camAtFour?.[0]).toMatchObject({ key: '00:00:00:00:00:01', value: 'sw1:port1', isNewInCurrentEvent: true })
    const arpAtEight = adapted.events.find((event) => event.traceSeq === '8')?.tablesSnapshot.pc2.arp
    expect(arpAtEight?.[0]).toMatchObject({ key: '192.168.1.10', value: '00:00:00:00:00:01', isNewInCurrentEvent: true })
  })

  it('marks table history incomplete when retention declares capacity loss', () => {
    const truncated = parsePresentationSnapshot({
      ...fixture,
      trace_retention: {
        total_emitted_ever: '50',
        dropped_by_capacity: '23',
        oldest_available_trace_seq: '1',
        newest_available_trace_seq: '27',
      },
    })
    const adapted = adaptPresentationSnapshot(truncated)

    expect(adapted.isTruncated).toBe(true)
    expect(adapted.droppedByCapacity).toBe(23n)
    expect(adapted.events[0].tablesSnapshot.pc1.incomplete).toBe(true)
  })

  it('removes ARP and CAM entries when the emitted expiration deltas arrive', () => {
    const snapshot = parsePresentationSnapshot({
      ...fixture,
      trace_retention: {
        total_emitted_ever: '29',
        dropped_by_capacity: '0',
        oldest_available_trace_seq: '1',
        newest_available_trace_seq: '29',
      },
      events: [
        ...fixture.events,
        {
          trace_seq: '28', at_ns: '60000004300', kind: 'table_updated',
          actor: { device_id: 'pc1', interface_id: null }, ingress: null, egress: null, destination: null, link_id: null,
          protocols: ['arp'], pdu: null, table_delta: { kind: 'arp_entry_expired', ip: '192.168.1.20' }, drop_reason: null, ping: null,
        },
        {
          trace_seq: '29', at_ns: '300000005400', kind: 'table_updated',
          actor: { device_id: 'sw1', interface_id: null }, ingress: null, egress: null, destination: null, link_id: null,
          protocols: ['ethernet'], pdu: null, table_delta: { kind: 'mac_entry_expired', mac: '00:00:00:00:00:01' }, drop_reason: null, ping: null,
        },
      ],
    })
    const adapted = adaptPresentationSnapshot(snapshot)

    expect(adapted.events.at(-2)?.tablesSnapshot.pc1.arp.map((entry) => entry.key)).not.toContain('192.168.1.20')
    expect(adapted.events.at(-1)?.tablesSnapshot.sw1.cam.map((entry) => entry.key)).not.toContain('00:00:00:00:00:01')
    expect(adapted.events.at(-1)?.tableDelta?.operation).toBe('expired')
  })
})
