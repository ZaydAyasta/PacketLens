<script setup lang="ts">
import type { PduStructuredViewModel } from '../../types/inspector'
import BadgeRealIllustrative from '../common/BadgeRealIllustrative.vue'

defineProps<{ pdu: PduStructuredViewModel | null; title: string }>()
</script>

<template>
  <section aria-live="polite" class="pdu-view">
    <h3>{{ title }}</h3>
    <p v-if="pdu === null">No hay PDU semántica emitida para este evento.</p>
    <template v-else><p>{{ pdu.summary }}</p><article v-for="block in pdu.blocks" :key="block.protocol" class="pdu-block"><h4>{{ block.title }}</h4>
      <dl><template v-for="field in block.fields" :key="field.name"><dt>{{ field.name }}</dt><dd>{{ field.value }} <BadgeRealIllustrative :fidelity="field.fidelity" /><small>{{ field.educationalNote }}</small></dd></template></dl>
    </article></template>
  </section>
</template>

<style scoped>
h3, h4 { margin: .25rem 0; }.pdu-block { border: 1px solid #dbeafe; border-radius: .4rem; margin: .5rem 0; padding: .5rem; }dl { display: grid; gap: .3rem; grid-template-columns: minmax(8rem, .7fr) minmax(12rem, 1.3fr); margin: 0; } dt { font-weight: 650; } dd { margin: 0; } small { color: #475569; display: block; margin-top: .15rem; }
</style>
