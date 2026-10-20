<script setup lang="ts">
import type { SnapshotSource } from '../../types/inspector'

const props = defineProps<{
  isTruncated: boolean
  droppedByCapacity: bigint
  source: SnapshotSource
  note?: string
}>()

const sourceLabel: Record<SnapshotSource, string> = {
  saved_engine_fixture: 'Snapshot guardado generado por el motor',
  live_engine: 'Ejecución viva del motor Rust',
  synthetic_fixture: 'Fixture sintético demostrativo',
}
</script>

<template>
  <section class="trace-banner" aria-live="polite">
    <p class="source"><strong>Procedencia:</strong> {{ sourceLabel[props.source] }}</p>
    <p v-if="props.source === 'synthetic_fixture'" class="synthetic"><strong>Fixture sintético:</strong> {{ props.note ?? 'No representa una ejecución del motor.' }}</p>
    <p v-if="props.isTruncated" class="truncated" role="alert"><strong>Traza truncada:</strong> se descartaron {{ props.droppedByCapacity.toString() }} eventos por capacidad; el estado histórico de tablas puede ser incompleto.</p>
  </section>
</template>

<style scoped>
.trace-banner { background: #f8fafc; border-inline-start: 4px solid #475569; color: #1e293b; margin: .75rem 0; padding: .5rem .75rem; }
p { margin: .2rem 0; }
.synthetic { color: #7e22ce; }.truncated { color: #991b1b; }
</style>
