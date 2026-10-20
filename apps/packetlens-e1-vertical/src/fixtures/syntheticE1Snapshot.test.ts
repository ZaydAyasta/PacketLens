import { describe, expect, it } from 'vitest'
import { parsePresentationSnapshot } from '../contracts/presentationContract'
import { SYNTHETIC_E1_SNAPSHOT } from './syntheticE1Snapshot'

describe('synthetic E1 snapshot', () => {
  it('is a valid explicitly illustrative presentation/v1 snapshot', () => {
    const snapshot = parsePresentationSnapshot(SYNTHETIC_E1_SNAPSHOT)
    expect(snapshot.run_label).toBe('synthetic-demonstration')
    expect(snapshot.events).toHaveLength(3)
    expect(snapshot.trace_retention.total_emitted_ever).toBe('3')
  })
})
