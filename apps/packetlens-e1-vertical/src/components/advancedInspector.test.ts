import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import fixture from '../fixtures/e1-s2-cold.presentation.json'
import { adaptPresentationSnapshot } from '../adapters/presentationAdapter'
import { parsePresentationSnapshot } from '../contracts/presentationContract'
import EventListView from './timeline/EventListView.vue'
import PduInspector from './inspector/PduInspector.vue'
import TablesView from './inspector/TablesView.vue'
import TopologyCanvas from './topology/TopologyCanvas.vue'
import TraceRetentionBanner from './common/TraceRetentionBanner.vue'

const adapted = adaptPresentationSnapshot(parsePresentationSnapshot(fixture))

describe('advanced inspector components', () => {
  it('shows all real events and emits selection by decimal trace sequence', async () => {
    const wrapper = mount(EventListView, {
      props: { events: adapted.events, selectedSeq: '1', filterArp: true, filterIcmp: true, filterDevice: 'ALL' },
    })

    expect(wrapper.get('[data-testid="event-count"]').text()).toContain('27')
    await wrapper.get('[data-trace-seq="2"]').trigger('click')
    expect(wrapper.emitted('select')?.[0]).toEqual(['2'])
  })

  it('renders topology and preserves PDU provenance and table deltas', () => {
    const topology = mount(TopologyCanvas, {
      props: { topology: adapted.topology, event: adapted.events[1], activeDeviceFilter: 'ALL' },
    })
    expect(topology.text()).toContain('PC1')
    expect(topology.text()).toContain('SW1')

    const inspector = mount(PduInspector, { props: { event: adapted.events[1] } })
    expect(inspector.text()).toContain('PDU')
    expect(inspector.text()).toContain('Dato del motor')
    expect(inspector.text()).toContain('No modelado en E1')

    const tables = mount(TablesView, { props: { event: adapted.events[3] } })
    expect(tables.text()).toContain('CAM')
    expect(tables.text()).toContain('00:00:00:00:00:01')
  })

  it('makes retention and synthetic provenance explicit', () => {
    const wrapper = mount(TraceRetentionBanner, {
      props: { isTruncated: true, droppedByCapacity: 9007199254740993n, source: 'synthetic_fixture', note: 'Demostración.' },
    })
    expect(wrapper.text()).toContain('Traza truncada')
    expect(wrapper.text()).toContain('9007199254740993')
    expect(wrapper.text()).toContain('Fixture sintético')
  })
})
