import { computed, ref } from 'vue'
import type { PresentationEvent, SimulationPresentationSnapshot } from '../contracts/presentation'

export interface EventFilter { readonly deviceId?: string; readonly protocol?: string; readonly kind?: string }

export function createPresentationState(snapshot: SimulationPresentationSnapshot) {
  const selectedTraceSeq = ref<string | null>(snapshot.events[0]?.trace_seq ?? null)
  const selected = computed(() => snapshot.events.find((event) => event.trace_seq === selectedTraceSeq.value) ?? null)

  function filteredEvents(filter: EventFilter = {}): readonly PresentationEvent[] {
    return snapshot.events.filter((event) =>
      (filter.deviceId === undefined || event.actor.device_id === filter.deviceId)
      && (filter.protocol === undefined || event.protocols.includes(filter.protocol))
      && (filter.kind === undefined || event.kind === filter.kind))
  }

  function select(traceSeq: string): void {
    if (snapshot.events.some((event) => event.trace_seq === traceSeq)) selectedTraceSeq.value = traceSeq
  }

  function previous(): void { move(-1) }
  function next(): void { move(1) }
  function move(delta: number): void {
    const index = snapshot.events.findIndex((event) => event.trace_seq === selectedTraceSeq.value)
    const target = snapshot.events[index + delta]
    if (target !== undefined) selectedTraceSeq.value = target.trace_seq
  }

  return { snapshot, selected, selectedTraceSeq, filteredEvents, select, previous, next }
}
