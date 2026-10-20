import { computed, isRef, ref, watch, type Ref } from 'vue'
import { adaptPresentationSnapshot } from '../adapters/presentationAdapter'
import type { SimulationPresentationSnapshot } from '../contracts/presentation'
import type { ProtocolType, SnapshotSource } from '../types/inspector'

export interface PresentationSession {
  readonly snapshot: SimulationPresentationSnapshot
  readonly source: SnapshotSource
}

type PresentationSessionInput = Ref<PresentationSession> | PresentationSession

/**
 * Local, read-only navigation state for a validated presentation snapshot.
 * Filters and cursor changes never call or modify the simulation engine.
 */
export function useSimulation(input: PresentationSessionInput) {
  const session = computed<PresentationSession>(() => isRef(input) ? input.value : input)
  const simulationData = computed(() => adaptPresentationSnapshot(session.value.snapshot))
  const rawEvents = computed(() => simulationData.value.events)
  const source = computed(() => session.value.source)
  const filterArp = ref(true)
  const filterIcmp = ref(true)
  const filterDevice = ref('ALL')
  const selectedTraceSeq = ref(rawEvents.value[0]?.traceSeq ?? '')
  const isPlaying = ref(false)
  const playSpeedMs = ref(1_000)
  let timer: ReturnType<typeof setInterval> | null = null

  const filteredEvents = computed(() => rawEvents.value.filter((event) => {
    if (!filterArp.value && event.protocols.includes('arp')) return false
    if (!filterIcmp.value && event.protocols.includes('icmp')) return false
    return filterDevice.value === 'ALL' || event.deviceId === filterDevice.value
  }))

  const currentEvent = computed(() => (
    filteredEvents.value.find((event) => event.traceSeq === selectedTraceSeq.value)
      ?? filteredEvents.value[0]
      ?? null
  ))

  const currentVisibleIndex = computed(() => currentEvent.value === null
    ? -1
    : filteredEvents.value.findIndex((event) => event.traceSeq === currentEvent.value?.traceSeq))

  watch(session, () => {
    pause()
    selectedTraceSeq.value = rawEvents.value[0]?.traceSeq ?? ''
    filterArp.value = true
    filterIcmp.value = true
    filterDevice.value = 'ALL'
  })

  function selectEvent(traceSeq: string): void {
    if (rawEvents.value.some((event) => event.traceSeq === traceSeq)) selectedTraceSeq.value = traceSeq
  }

  function nextStep(): void {
    const index = currentVisibleIndex.value
    if (index < 0 || index >= filteredEvents.value.length - 1) {
      pause()
      return
    }
    selectedTraceSeq.value = filteredEvents.value[index + 1].traceSeq
  }

  function prevStep(): void {
    const index = currentVisibleIndex.value
    if (index > 0) selectedTraceSeq.value = filteredEvents.value[index - 1].traceSeq
  }

  function play(): void {
    if (isPlaying.value || filteredEvents.value.length === 0) return
    if (currentVisibleIndex.value >= filteredEvents.value.length - 1) {
      selectedTraceSeq.value = filteredEvents.value[0].traceSeq
    }
    isPlaying.value = true
    timer = setInterval(nextStep, playSpeedMs.value)
  }

  function pause(): void {
    isPlaying.value = false
    if (timer !== null) {
      clearInterval(timer)
      timer = null
    }
  }

  function togglePlay(): void {
    if (isPlaying.value) pause()
    else play()
  }

  function reset(): void {
    pause()
    selectedTraceSeq.value = filteredEvents.value[0]?.traceSeq ?? ''
  }

  function toggleFilter(protocol: Extract<ProtocolType, 'ARP' | 'ICMP'>): void {
    if (protocol === 'ARP') filterArp.value = !filterArp.value
    if (protocol === 'ICMP') filterIcmp.value = !filterIcmp.value
  }

  function setDeviceFilter(deviceId: string): void {
    filterDevice.value = deviceId
  }

  return {
    simulationData,
    source,
    rawEvents,
    topology: computed(() => simulationData.value.topology),
    isTruncated: computed(() => simulationData.value.isTruncated),
    droppedByCapacity: computed(() => simulationData.value.droppedByCapacity),
    filterArp,
    filterIcmp,
    filterDevice,
    filteredEvents,
    selectedTraceSeq,
    currentEvent,
    currentVisibleIndex,
    isPlaying,
    playSpeedMs,
    selectEvent,
    nextStep,
    prevStep,
    play,
    pause,
    togglePlay,
    reset,
    dispose: pause,
    toggleFilter,
    setDeviceFilter,
  }
}
