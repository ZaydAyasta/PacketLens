<script setup lang="ts">
import type { DataFidelityKind } from '../../types/inspector'

const props = defineProps<{ fidelity: DataFidelityKind }>()

const labels: Record<DataFidelityKind, string> = {
  engine_fact: 'Dato del motor',
  didactic_context: 'Contexto didáctico',
  unmodeled: 'No modelado en E1',
}

const classFor: Record<DataFidelityKind, string> = {
  engine_fact: 'badge-engine badge-real',
  didactic_context: 'badge-didactic badge-illustrative',
  unmodeled: 'badge-unmodeled badge-illustrative',
}
</script>

<template>
  <span :class="classFor[props.fidelity]">{{ labels[props.fidelity] }}</span>
</template>

<style scoped>
span { border-radius: .25rem; display: inline-block; font-size: .72rem; font-weight: 650; padding: .15rem .4rem; }
.badge-engine { background: #dcfce7; color: #166534; }
.badge-didactic { background: #e0f2fe; color: #075985; }
.badge-unmodeled { background: #f3e8ff; color: #6b21a8; }
</style>
