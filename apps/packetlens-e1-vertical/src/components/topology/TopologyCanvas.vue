<script setup lang="ts">
import type { InspectorEventViewModel, TopologyViewModel } from '../../types/inspector'

const props = defineProps<{
  topology: TopologyViewModel
  event: InspectorEventViewModel | null
  activeDeviceFilter: string
}>()
const emit = defineEmits<{ selectDevice: [deviceId: string] }>()

function toggleDevice(deviceId: string): void {
  emit('selectDevice', props.activeDeviceFilter === deviceId ? 'ALL' : deviceId)
}

function pointFor(interfaceId: string): { x: number; y: number } {
  const deviceId = props.topology.interfaces.find((networkInterface) => networkInterface.id === interfaceId)?.deviceId
  const node = props.topology.nodes.find((candidate) => candidate.id === deviceId)
  return node === undefined ? { x: 50, y: 50 } : node
}
</script>

<template>
  <section class="topology" aria-labelledby="topology-title">
    <div class="section-heading"><h2 id="topology-title">Topología observada</h2><p v-if="props.event">Evento #{{ props.event.traceSeq }} · {{ props.event.timestampFormatted }}</p></div>
    <svg viewBox="0 0 100 100" role="img" aria-label="Topología de dispositivos y enlaces">
      <line v-for="link in props.topology.links" :key="link.id" :x1="pointFor(link.aInterfaceId).x" :y1="pointFor(link.aInterfaceId).y" :x2="pointFor(link.bInterfaceId).x" :y2="pointFor(link.bInterfaceId).y" class="link" :class="{ active: props.event?.linkId === link.id }" />
      <g v-for="node in props.topology.nodes" :key="node.id" class="node" :class="{ active: props.activeDeviceFilter === node.id }" :transform="`translate(${node.x} ${node.y})`" @click="toggleDevice(node.id)">
        <rect class="node-card" x="-9" y="-6" width="18" height="12" rx="2" tabindex="0" role="button" :aria-label="`Filtrar por ${node.label}`" @keydown.enter.prevent="toggleDevice(node.id)" @keydown.space.prevent="toggleDevice(node.id)" />
        <text text-anchor="middle" y="1">{{ node.label }}</text>
      </g>
    </svg>
    <p v-if="props.event" class="event-summary">{{ props.event.summary }}</p>
  </section>
</template>

<style scoped>
.topology { background: #f8fafc; border: 1px solid #dbeafe; border-radius: .4rem; padding: .75rem; }.section-heading { align-items: baseline; display: flex; justify-content: space-between; }.section-heading p, h2 { margin: 0; } h2 { font-size: 1rem; } svg { min-height: 220px; width: 100%; }.link { stroke: #94a3b8; stroke-width: 1; }.link.active { stroke: #0284c7; stroke-width: 2; }.node { cursor: pointer; }.node-card { fill: #fff; stroke: #475569; stroke-width: .5; }.node.active .node-card { fill: #e0f2fe; stroke: #0369a1; stroke-width: 1; } text { font-size: 3.5px; pointer-events: none; }.event-summary { font-size: .85rem; }
</style>
