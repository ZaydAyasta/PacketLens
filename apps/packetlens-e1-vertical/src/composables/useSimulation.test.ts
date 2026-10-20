import { describe, expect, it, vi } from 'vitest'
import { ref } from 'vue'
import fixture from '../fixtures/e1-s2-cold.presentation.json'
import { parsePresentationSnapshot } from '../contracts/presentationContract'
import { useSimulation } from './useSimulation'

describe('useSimulation — read-only presentation navigation', () => {
  it('filters and navigates the 27-event engine snapshot without mutating it', () => {
    const snapshot = parsePresentationSnapshot(fixture)
    const session = ref({ snapshot, source: 'saved_engine_fixture' as const })
    const simulation = useSimulation(session)

    expect(simulation.rawEvents.value).toHaveLength(27)
    expect(simulation.selectedTraceSeq.value).toBe('1')
    simulation.nextStep()
    expect(simulation.selectedTraceSeq.value).toBe('2')

    simulation.toggleFilter('ARP')
    expect(simulation.rawEvents.value).toHaveLength(27)
    expect(simulation.filteredEvents.value).not.toHaveLength(27)

    simulation.setDeviceFilter('pc2')
    expect(simulation.filteredEvents.value.every((event) => event.deviceId === 'pc2')).toBe(true)
  })

  it('retains source provenance and stops playback at the visible boundary', () => {
    vi.useFakeTimers()
    const snapshot = parsePresentationSnapshot(fixture)
    const simulation = useSimulation(ref({ snapshot, source: 'live_engine' as const }))

    expect(simulation.source.value).toBe('live_engine')
    simulation.playSpeedMs.value = 200
    simulation.play()
    vi.advanceTimersByTime(5_400)
    expect(simulation.selectedTraceSeq.value).toBe('27')
    expect(simulation.isPlaying.value).toBe(false)
    simulation.dispose()
    vi.useRealTimers()
  })
})
