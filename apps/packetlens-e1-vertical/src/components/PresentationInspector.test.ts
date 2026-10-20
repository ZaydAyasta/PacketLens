import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import fixture from '../fixtures/e1-s2-cold.presentation.json'
import { parsePresentationSnapshot } from '../contracts/presentationContract'
import PresentationInspector from './PresentationInspector.vue'

describe('PresentationInspector', () => {
  it('uses all 27 engine events, selects by trace_seq, and shows its semantic PDU', async () => {
    const wrapper = mount(PresentationInspector, { props: { snapshot: parsePresentationSnapshot(fixture) } })

    await wrapper.get('[data-trace-seq="17"]').trigger('click')

    expect(wrapper.findAll('[data-testid="trace-event"]')).toHaveLength(27)
    expect(wrapper.get('[data-testid="selected-trace-seq"]').text()).toBe('17')
    expect(wrapper.get('[data-testid="pdu"]').text()).toContain('192.168.1.10')
    expect(wrapper.get('[data-testid="pdu"]').text()).toContain('echo_request')
  })

  it('filters without altering trace retention and visually navigates backward', async () => {
    const wrapper = mount(PresentationInspector, { props: { snapshot: parsePresentationSnapshot(fixture) } })

    await wrapper.get('[data-testid="device-filter"]').setValue('pc3')
    await wrapper.get('[data-trace-seq="10"]').trigger('click')
    await wrapper.get('[data-testid="previous"]').trigger('click')

    expect(wrapper.findAll('[data-testid="trace-event"]')).toHaveLength(2)
    expect(wrapper.get('[data-testid="selected-trace-seq"]').text()).toBe('9')
    expect(wrapper.get('[data-testid="retention"]').text()).toContain('27 / 0')
  })
})
