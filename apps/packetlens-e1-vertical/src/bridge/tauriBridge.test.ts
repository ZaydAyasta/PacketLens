import { describe, expect, it, vi } from 'vitest'
import fixture from '../fixtures/e1-s2-cold.presentation.json'
import { ContractVersionMismatchError, createTauriPresentationBridge } from './tauriBridge'

describe('createTauriPresentationBridge', () => {
  it('checks the version before parsing a real engine snapshot', async () => {
    const invoke = vi.fn()
      .mockResolvedValueOnce('presentation/v1')
      .mockResolvedValueOnce(JSON.stringify(fixture))
    const bridge = createTauriPresentationBridge(invoke)

    const snapshot = await bridge.runE1S2Cold()

    expect(snapshot.events).toHaveLength(27)
    expect(invoke.mock.calls).toEqual([['contract_version'], ['run_e1_s2_cold']])
  })

  it('rejects an unsupported bridge contract before requesting a run', async () => {
    const invoke = vi.fn().mockResolvedValue('presentation/v2')
    const bridge = createTauriPresentationBridge(invoke)

    await expect(bridge.runE1S2Cold()).rejects.toBeInstanceOf(ContractVersionMismatchError)
    expect(invoke.mock.calls).toEqual([['contract_version']])
  })

  it('rejects a non-text bridge result without accepting a partial snapshot', async () => {
    const invoke = vi.fn()
      .mockResolvedValueOnce('presentation/v1')
      .mockResolvedValueOnce({ events: [] })
    const bridge = createTauriPresentationBridge(invoke)

    await expect(bridge.runE1S2Cold()).rejects.toThrow('no devolvió JSON')
    expect(invoke.mock.calls).toEqual([['contract_version'], ['run_e1_s2_cold']])
  })

  it('rejects malformed engine JSON before it can reach presentation state', async () => {
    const invoke = vi.fn()
      .mockResolvedValueOnce('presentation/v1')
      .mockResolvedValueOnce('{not-json')
    const bridge = createTauriPresentationBridge(invoke)

    await expect(bridge.runE1S2Cold()).rejects.toThrow(SyntaxError)
  })
})
