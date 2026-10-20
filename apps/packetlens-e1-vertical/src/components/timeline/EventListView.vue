<script setup lang="ts">
import type { InspectorEventViewModel } from '../../types/inspector'

const props = defineProps<{
  events: readonly InspectorEventViewModel[]
  selectedSeq: string
  filterArp: boolean
  filterIcmp: boolean
  filterDevice: string
}>()
const emit = defineEmits<{
  select: [traceSeq: string]
  toggleFilter: [protocol: 'ARP' | 'ICMP']
  setDeviceFilter: [deviceId: string]
}>()

const devices = () => Array.from(new Set(props.events.map((event) => event.deviceId))).sort()

function select(traceSeq: string): void { emit('select', traceSeq) }
</script>

<template>
  <section aria-labelledby="event-list-title" class="timeline">
    <div class="section-heading"><h2 id="event-list-title">Eventos de la traza</h2><span data-testid="event-count">{{ props.events.length }} eventos visibles</span></div>
    <fieldset class="filters"><legend>Filtros de presentación</legend>
      <label><input class="arp-filter" type="checkbox" :checked="props.filterArp" @change="emit('toggleFilter', 'ARP')"> ARP</label>
      <label><input class="icmp-filter" type="checkbox" :checked="props.filterIcmp" @change="emit('toggleFilter', 'ICMP')"> ICMP</label>
      <label>Dispositivo <select class="device-filter-select" :value="props.filterDevice" @change="emit('setDeviceFilter', ($event.target as HTMLSelectElement).value)"><option value="ALL">Todos</option><option v-for="device in devices()" :key="device" :value="device">{{ device.toUpperCase() }}</option></select></label>
    </fieldset>
    <div class="table-wrap"><table><thead><tr><th>#</th><th>Tiempo</th><th>Dispositivo</th><th>Evento</th><th>Protocolo</th></tr></thead>
      <tbody><tr v-for="event in props.events" :key="event.traceSeq" :data-trace-seq="event.traceSeq" :class="{ selected: event.traceSeq === props.selectedSeq }" tabindex="0" @click="select(event.traceSeq)" @keydown.enter.prevent="select(event.traceSeq)" @keydown.space.prevent="select(event.traceSeq)">
        <td>{{ event.traceSeq }}</td><td>{{ event.timestampFormatted }}</td><td>{{ event.deviceId.toUpperCase() }}</td><td>{{ event.kind }}</td><td>{{ event.protocol }}</td>
      </tr></tbody>
    </table></div>
  </section>
</template>

<style scoped>
.timeline { margin-top: 1rem; }.section-heading { align-items: baseline; display: flex; justify-content: space-between; } h2 { font-size: 1rem; }.filters { border: 0; display: flex; flex-wrap: wrap; gap: .75rem; margin: 0; padding: .5rem 0; }.table-wrap { max-height: 20rem; overflow: auto; } table { border-collapse: collapse; width: 100%; } th { position: sticky; top: 0; } th, td { border-bottom: 1px solid #e2e8f0; font-size: .82rem; padding: .4rem; text-align: left; } tr { cursor: pointer; } tr.selected { background: #e0f2fe; outline: 2px solid #38bdf8; outline-offset: -2px; }
</style>
