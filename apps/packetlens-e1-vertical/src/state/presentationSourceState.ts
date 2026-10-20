import { ref } from 'vue'
import type { PresentationBridge } from '../bridge/tauriBridge'
import type { SimulationPresentationSnapshot } from '../contracts/presentation'
import type { SnapshotSource } from '../types/inspector'
import type { PresentationSession } from '../composables/useSimulation'

/**
 * Owns only presentation-source selection. The bridge owns version checking
 * and runtime parsing; this state replaces the displayed snapshot only after
 * the bridge resolves a validated value.
 */
export function createPresentationSourceState(
  savedEngineFixture: SimulationPresentationSnapshot,
  syntheticFixture: SimulationPresentationSnapshot,
) {
  const session = ref<PresentationSession>({ snapshot: savedEngineFixture, source: 'saved_engine_fixture' })
  const selectedSource = ref<SnapshotSource>('saved_engine_fixture')
  const isRunning = ref(false)
  const error = ref<string | null>(null)

  function selectSource(source: SnapshotSource): void {
    selectedSource.value = source
    error.value = null
    if (source === 'saved_engine_fixture') session.value = { snapshot: savedEngineFixture, source }
    if (source === 'synthetic_fixture') session.value = { snapshot: syntheticFixture, source }
  }

  async function runLive(bridge: PresentationBridge): Promise<void> {
    selectedSource.value = 'live_engine'
    isRunning.value = true
    error.value = null
    try {
      const snapshot = await bridge.runE1S2Cold()
      session.value = { snapshot, source: 'live_engine' }
    } catch (caught) {
      error.value = caught instanceof Error ? caught.message : String(caught)
    } finally {
      isRunning.value = false
    }
  }

  return { session, selectedSource, isRunning, error, selectSource, runLive }
}
