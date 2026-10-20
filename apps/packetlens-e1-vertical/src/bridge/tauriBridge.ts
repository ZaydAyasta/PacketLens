import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { PRESENTATION_SCHEMA_VERSION, type SimulationPresentationSnapshot } from '../contracts/presentation'
import { parsePresentationSnapshot } from '../contracts/presentationContract'

export type Invoke = (command: string) => Promise<unknown>

export interface PresentationBridge {
  runE1S2Cold(): Promise<SimulationPresentationSnapshot>
}

export class ContractVersionMismatchError extends Error {
  constructor(readonly received: string) {
    super(`El bridge expone ${received}; se esperaba ${PRESENTATION_SCHEMA_VERSION}.`)
    this.name = 'ContractVersionMismatchError'
  }
}

export function createTauriPresentationBridge(invoke: Invoke): PresentationBridge {
  return {
    async runE1S2Cold(): Promise<SimulationPresentationSnapshot> {
      const version = await invoke('contract_version')
      if (version !== PRESENTATION_SCHEMA_VERSION) throw new ContractVersionMismatchError(String(version))
      const serialized = await invoke('run_e1_s2_cold')
      if (typeof serialized !== 'string') throw new Error('El bridge no devolvió JSON de presentación.')
      return parsePresentationSnapshot(JSON.parse(serialized))
    },
  }
}

export const tauriPresentationBridge = createTauriPresentationBridge(tauriInvoke)
