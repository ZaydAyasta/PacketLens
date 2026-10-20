<script setup lang="ts">
import type { SnapshotSource } from '../../types/inspector'

const props = defineProps<{
  source: SnapshotSource
  isPlaying: boolean
  canPrev: boolean
  canNext: boolean
  busy: boolean
}>()

const emit = defineEmits<{
  previous: []
  next: []
  togglePlay: []
  reset: []
  selectSource: [source: SnapshotSource]
  executeRust: []
}>()
</script>

<template>
  <header class="app-header">
    <div><p class="eyebrow">PacketLens · E1</p><h1>Inspector educativo de traza</h1></div>
    <nav aria-label="Controles de la traza" class="controls">
      <label>Fuente
        <select :value="props.source" @change="emit('selectSource', ($event.target as HTMLSelectElement).value as SnapshotSource)">
          <option value="saved_engine_fixture">Snapshot guardado</option>
          <option value="live_engine">Ejecución viva</option>
          <option value="synthetic_fixture">Fixture sintético</option>
        </select>
      </label>
      <button type="button" :disabled="props.busy" @click="emit('executeRust')">{{ props.busy ? 'Ejecutando…' : 'Ejecutar motor Rust' }}</button>
      <button type="button" :disabled="!props.canPrev" @click="emit('previous')">Anterior</button>
      <button type="button" :disabled="!props.canNext && !props.isPlaying" @click="emit('togglePlay')">{{ props.isPlaying ? 'Pausar' : 'Reproducir' }}</button>
      <button type="button" :disabled="!props.canNext" @click="emit('next')">Siguiente</button>
      <button type="button" @click="emit('reset')">Reiniciar vista</button>
    </nav>
  </header>
</template>

<style scoped>
.app-header { align-items: center; border-bottom: 1px solid #cbd5e1; display: flex; gap: 1rem; justify-content: space-between; padding: 1rem 0; }
h1 { font-size: 1.5rem; margin: 0; }.eyebrow { color: #0369a1; font-size: .8rem; font-weight: 700; letter-spacing: .08em; margin: 0 0 .15rem; text-transform: uppercase; }
.controls { align-items: end; display: flex; flex-wrap: wrap; gap: .4rem; }.controls label { display: grid; font-size: .75rem; gap: .15rem; } button, select { min-height: 2rem; }
</style>
