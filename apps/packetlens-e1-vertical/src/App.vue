<script setup lang="ts">
import { onUnmounted } from 'vue'
import { tauriPresentationBridge } from './bridge/tauriBridge'
import { useSimulation } from './composables/useSimulation'
import AppHeader from './components/layout/AppHeader.vue'
import TraceRetentionBanner from './components/common/TraceRetentionBanner.vue'
import TopologyCanvas from './components/topology/TopologyCanvas.vue'
import EventListView from './components/timeline/EventListView.vue'
import PduInspector from './components/inspector/PduInspector.vue'
import { parsePresentationSnapshot } from './contracts/presentationContract'
import savedEngineFixture from './fixtures/e1-s2-cold.presentation.json'
import { SYNTHETIC_E1_SNAPSHOT } from './fixtures/syntheticE1Snapshot'
import { createPresentationSourceState } from './state/presentationSourceState'
import type { SnapshotSource } from './types/inspector'

const sourceState = createPresentationSourceState(
  parsePresentationSnapshot(savedEngineFixture),
  parsePresentationSnapshot(SYNTHETIC_E1_SNAPSHOT),
)
const simulation = useSimulation(sourceState.session)

function selectSource(source: SnapshotSource): void {
  sourceState.selectSource(source)
}

async function runRealE1S2Cold(): Promise<void> {
  await sourceState.runLive(tauriPresentationBridge)
}

onUnmounted(simulation.dispose)
</script>

<template>
  <main class="packetlens-app">
    <AppHeader
      :source="sourceState.selectedSource.value"
      :is-playing="simulation.isPlaying.value"
      :can-prev="simulation.currentVisibleIndex.value > 0"
      :can-next="simulation.currentVisibleIndex.value >= 0 && simulation.currentVisibleIndex.value < simulation.filteredEvents.value.length - 1"
      :busy="sourceState.isRunning.value"
      @previous="simulation.prevStep"
      @next="simulation.nextStep"
      @toggle-play="simulation.togglePlay"
      @reset="simulation.reset"
      @select-source="selectSource"
      @execute-rust="runRealE1S2Cold"
    />

    <TraceRetentionBanner
      :is-truncated="simulation.isTruncated.value"
      :dropped-by-capacity="simulation.droppedByCapacity.value"
      :source="sourceState.session.value.source"
      :note="sourceState.session.value.source === 'synthetic_fixture' ? 'Material de demostración; no procede del motor PacketLens.' : undefined"
    />
    <p v-if="sourceState.selectedSource.value === 'live_engine' && sourceState.session.value.source !== 'live_engine'" class="mode-note">Modo de ejecución viva seleccionado. Pulse “Ejecutar motor Rust” para reemplazar la vista cuando el bridge devuelva un snapshot válido.</p>
    <p v-if="sourceState.error.value" class="execution-error" role="alert">No se reemplazó el snapshot actual: {{ sourceState.error.value }}</p>

    <section class="summary" aria-label="Resumen de la ejecución">
      <strong>{{ simulation.simulationData.value.scenarioId }} · {{ simulation.simulationData.value.runLabel }}</strong>
      <span>Resultado: {{ simulation.simulationData.value.isQuiescent ? 'Quiescent' : 'No quiescent' }}</span>
      <span>Tiempo virtual final: {{ simulation.simulationData.value.finalVirtualTimeFormatted }}</span>
    </section>

    <section class="workspace">
      <div class="trace-pane">
        <TopologyCanvas :topology="simulation.topology.value" :event="simulation.currentEvent.value" :active-device-filter="simulation.filterDevice.value" @select-device="simulation.setDeviceFilter" />
        <EventListView :events="simulation.filteredEvents.value" :selected-seq="simulation.currentEvent.value?.traceSeq ?? ''" :filter-arp="simulation.filterArp.value" :filter-icmp="simulation.filterIcmp.value" :filter-device="simulation.filterDevice.value" @select="simulation.selectEvent" @toggle-filter="simulation.toggleFilter" @set-device-filter="simulation.setDeviceFilter" />
      </div>
      <PduInspector :event="simulation.currentEvent.value" />
    </section>
  </main>
</template>

<style scoped>
.packetlens-app { color: #0f172a; font-family: Inter, ui-sans-serif, system-ui, sans-serif; margin: 0 auto; max-width: 1440px; padding: 0 1rem 2rem; }.summary { background: #f8fafc; display: flex; flex-wrap: wrap; gap: 1rem; margin-bottom: .75rem; padding: .5rem .75rem; }.workspace { align-items: start; display: grid; gap: 1rem; grid-template-columns: minmax(0, 1.5fr) minmax(18rem, .85fr); }.trace-pane { min-width: 0; }.execution-error { background: #fef2f2; border-inline-start: 4px solid #dc2626; color: #991b1b; padding: .6rem .75rem; }.mode-note { color: #075985; } @media (max-width: 900px) { .workspace { grid-template-columns: 1fr; } }
</style>
