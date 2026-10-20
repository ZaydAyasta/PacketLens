<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { InspectorEventViewModel } from '../../types/inspector'
import OsiLayersView from './OsiLayersView.vue'
import PduStructuredView from './PduStructuredView.vue'
import TablesView from './TablesView.vue'

const props = defineProps<{ event: InspectorEventViewModel | null }>()
const tab = ref<'pdu' | 'reading' | 'tables'>('pdu')
const activePdu = computed(() => props.event?.pduOut ?? props.event?.pduIn ?? null)
watch(() => props.event?.traceSeq, () => { tab.value = 'pdu' })
</script>

<template>
  <aside class="inspector" aria-label="Inspector de evento"><div role="tablist" aria-label="Detalles del evento">
    <button type="button" class="tab-btn" role="tab" :aria-selected="tab === 'pdu'" @click="tab = 'pdu'">PDU</button>
    <button type="button" class="tab-btn" role="tab" :aria-selected="tab === 'reading'" @click="tab = 'reading'">Lectura didáctica</button>
    <button type="button" class="tab-btn" role="tab" :aria-selected="tab === 'tables'" @click="tab = 'tables'">Tablas</button>
  </div>
  <PduStructuredView v-if="tab === 'pdu'" :pdu="activePdu" title="PDU semántica" />
  <OsiLayersView v-else-if="tab === 'reading'" :event="props.event" />
  <TablesView v-else :event="props.event" />
  </aside>
</template>

<style scoped>.inspector { background: #fff; border: 1px solid #cbd5e1; border-radius: .4rem; padding: .75rem; }.tab-btn[aria-selected="true"] { background: #0c4a6e; color: #fff; }</style>
