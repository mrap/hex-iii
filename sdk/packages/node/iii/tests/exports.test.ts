import { beforeAll, describe, expect, it, vi } from 'vitest'
import { registerWorker, Logger } from '../src/index'
import { iii } from './utils'

beforeAll(() => {
  vi.spyOn(iii, 'shutdown').mockResolvedValue(undefined)
})

describe('Package Exports', () => {
  it('should export main SDK symbols', () => {
    expect(registerWorker).toBeDefined()
    expect(typeof registerWorker).toBe('function')
    expect(Logger).toBeDefined()
  })

  it('should import stream module', async () => {
    await expect(import('../src/stream')).resolves.toBeDefined()
  })

  it('should import state module', async () => {
    const stateModule = await import('../src/state')
    expect(stateModule).toBeDefined()
    expect(stateModule.StateEventType).toBeDefined()
    expect(Object.keys(stateModule).length).toBeGreaterThan(0)
  })

  it('should import types module', async () => {
    // `iii-sdk/types` holds type-only re-exports (MessageType, IIIConnectionState)
    // relocated from the root in 0.18.0. The compile-time boundary is enforced by
    // tests/relocation-guard.ts; this just confirms the entry point resolves.
    await expect(import('../src/public-types')).resolves.toBeDefined()
  })
})
