import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import fixture from './fixtures/e1-s2-cold.presentation.json'
import App from './App.vue'

const runE1S2Cold = vi.hoisted(() => vi.fn())

async function settle(): Promise<void> {
  await Promise.resolve()
  await nextTick()
}

vi.mock('./bridge/tauriBridge', () => ({
  tauriPresentationBridge: { runE1S2Cold },
}))

describe('App advanced inspector integration', () => {
  beforeEach(() => { runE1S2Cold.mockReset() })

  it('opens with the saved engine-generated 27-event fixture and can select the explicit synthetic source', async () => {
    const wrapper = mount(App)
    expect(wrapper.text()).toContain('27 eventos visibles')
    expect(wrapper.text()).toContain('Snapshot guardado generado por el motor')

    await wrapper.get('select').setValue('synthetic_fixture')
    expect(wrapper.text()).toContain('Fixture sintético')
  })

  it('uses the existing bridge for a live run and preserves the prior view on error', async () => {
    runE1S2Cold.mockRejectedValueOnce(new Error('motor no disponible'))
    const wrapper = mount(App)

    await wrapper.get('button').trigger('click')
    await settle()
    expect(runE1S2Cold).toHaveBeenCalledTimes(1)
    expect(wrapper.text()).toContain('motor no disponible')
    expect(wrapper.text()).toContain('27 eventos visibles')
    expect(wrapper.text()).toContain('Snapshot guardado generado por el motor')

    runE1S2Cold.mockResolvedValueOnce(fixture)
    await wrapper.get('button').trigger('click')
    await settle()
    expect(wrapper.text()).toContain('Ejecución viva del motor Rust')
  })
})
