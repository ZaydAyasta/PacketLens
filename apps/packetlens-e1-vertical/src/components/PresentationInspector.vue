<script setup lang="ts">
import { computed, ref } from 'vue'
import type { SimulationPresentationSnapshot } from '../contracts/presentation'
import { createPresentationState } from '../state/presentationState'

const props = defineProps<{ snapshot: SimulationPresentationSnapshot }>()
const state = createPresentationState(props.snapshot)
const { selected, select, previous, next } = state
const deviceFilter = ref('')
const visibleEvents = computed(() => state.filteredEvents(deviceFilter.value ? { deviceId: deviceFilter.value } : {}))
const deviceIds = [...new Set(props.snapshot.topology.devices.map((device) => device.id))]

function json(value: unknown): string {
  return value === null ? 'Sin PDU emitida' : JSON.stringify(value, null, 2)
}
</script>

<template>
  <section aria-label="Inspector de traza E1">
    <p data-testid="retention">
      Retención: {{ snapshot.trace_retention.total_emitted_ever }} / {{ snapshot.trace_retention.dropped_by_capacity }}
    </p>
    <label>
      Dispositivo
      <select v-model="deviceFilter" data-testid="device-filter">
        <option value="">Todos</option>
        <option v-for="deviceId in deviceIds" :key="deviceId" :value="deviceId">{{ deviceId }}</option>
      </select>
    </label>
    <ol aria-label="Eventos de la traza">
      <li v-for="event in visibleEvents" :key="event.trace_seq" data-testid="trace-event">
        <button type="button" :data-trace-seq="event.trace_seq" @click="select(event.trace_seq)">
          #{{ event.trace_seq }} · {{ event.at_ns }} ns · {{ event.kind }}
        </button>
      </li>
    </ol>
    <section v-if="selected" aria-label="Detalle del evento">
      <button type="button" data-testid="previous" @click="previous">Anterior</button>
      <button type="button" data-testid="next" @click="next">Siguiente</button>
      <p data-testid="selected-trace-seq">{{ selected.trace_seq }}</p>
      <pre data-testid="pdu">{{ json(selected.pdu?.pdu ?? null) }}</pre>
    </section>
  </section>
</template>
