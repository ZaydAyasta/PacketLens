<script setup lang="ts">
import type { InspectorEventViewModel } from '../../types/inspector'

defineProps<{ event: InspectorEventViewModel | null }>()
</script>

<template>
  <section class="didactic-reading"><h3>Lectura pedagógica de la traza</h3>
    <p v-if="event === null">Seleccione un evento para consultar su explicación.</p>
    <template v-else><p>{{ event.summary }}</p><p>{{ event.pedagogicalRationale }}</p><p class="caveat">Esta lectura no es una decisión OSI emitida por el motor. Los hechos del motor permanecen en los campos y deltas del snapshot.</p></template>
  </section>
</template>

<style scoped>.didactic-reading { background: #f8fafc; border-inline-start: 4px solid #0ea5e9; padding: .5rem .75rem; }.caveat { color: #475569; font-size: .85rem; }</style>
