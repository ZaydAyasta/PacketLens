import { describe, expect, it } from 'vitest'
import fixture from '../fixtures/e1-s2-cold.presentation.json'
import { parsePresentationSnapshot } from '../contracts/presentationContract'
import { createPresentationSourceState } from './presentationSourceState'

describe('presentation source state', () => {
  it('distinguishes saved engine output, synthetic data, and a successful live run', async () => {
    const saved = parsePresentationSnapshot(fixture)
    const synthetic = { ...saved, run_label: 'synthetic-demo' }
    const state = createPresentationSourceState(saved, synthetic)

    state.selectSource('synthetic_fixture')
    expect(state.session.value.source).toBe('synthetic_fixture')
    expect(state.session.value.snapshot.run_label).toBe('synthetic-demo')

    const syntheticSession = state.session.value
    state.selectSource('live_engine')
    expect(state.session.value).toBe(syntheticSession)

    await state.runLive({ runE1S2Cold: async () => saved })
    expect(state.session.value.source).toBe('live_engine')
    expect(state.session.value.snapshot.events).toHaveLength(27)
  })

  it('retains the last valid snapshot and exposes an error when the live bridge fails', async () => {
    const saved = parsePresentationSnapshot(fixture)
    const state = createPresentationSourceState(saved, { ...saved, run_label: 'synthetic-demo' })
    const before = state.session.value.snapshot

    await state.runLive({ runE1S2Cold: async () => { throw new Error('bridge unavailable') } })

    expect(state.session.value.snapshot).toBe(before)
    expect(state.error.value).toContain('bridge unavailable')
    expect(state.isRunning.value).toBe(false)
  })
})
