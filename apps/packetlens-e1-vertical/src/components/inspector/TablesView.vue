<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { InspectorEventViewModel } from '../../types/inspector'

const props = defineProps<{ event: InspectorEventViewModel | null }>()
const selectedDevice = ref('')
const devices = computed(() => props.event === null ? [] : Object.keys(props.event.tablesSnapshot))
watch(() => props.event?.traceSeq, () => {
  if (!devices.value.includes(selectedDevice.value)) selectedDevice.value = props.event?.deviceId ?? devices.value[0] ?? ''
}, { immediate: true })
const tables = computed(() => props.event?.tablesSnapshot[selectedDevice.value] ?? null)
</script>

<template>
  <section><h3>Tablas observadas</h3>
    <p v-if="event?.tableDelta" class="delta"><strong>Cambio en tabla en este evento:</strong> {{ event.tableDelta.explanation }}</p>
    <p v-if="tables?.incomplete" role="note">El historial fue truncado; estas tablas pueden no incluir entradas anteriores.</p>
    <div role="group" aria-label="Dispositivo de las tablas"><button v-for="device in devices" :key="device" type="button" class="dev-btn" :aria-pressed="selectedDevice === device" @click="selectedDevice = device">{{ device.toUpperCase() }}</button></div>
    <p v-if="tables === null">No hay tablas disponibles.</p>
    <template v-else><article><h4>Caché ARP</h4><table><thead><tr><th>IP</th><th>MAC</th><th>Expira</th></tr></thead><tbody><tr v-for="row in tables.arp" :key="row.key" :class="{ new: row.isNewInCurrentEvent }"><td>{{ row.key }}</td><td>{{ row.value }}</td><td>{{ row.expiresAtNs ?? '—' }}</td></tr><tr v-if="tables.arp.length === 0"><td colspan="3">Sin entradas ARP.</td></tr></tbody></table></article>
    <article><h4>Tabla CAM</h4><table><thead><tr><th>MAC</th><th>Puerto</th><th>Expira</th></tr></thead><tbody><tr v-for="row in tables.cam" :key="row.key" :class="{ new: row.isNewInCurrentEvent }"><td>{{ row.key }}</td><td>{{ row.value }}</td><td>{{ row.expiresAtNs ?? '—' }}</td></tr><tr v-if="tables.cam.length === 0"><td colspan="3">Sin entradas CAM.</td></tr></tbody></table></article></template>
  </section>
</template>

<style scoped>
button { margin: 0 .3rem .5rem 0; }.delta { background: #ecfeff; padding: .5rem; } table { border-collapse: collapse; width: 100%; } th,td { border-bottom: 1px solid #e2e8f0; padding: .35rem; text-align: left; }.new { background: #ecfccb; }
</style>
