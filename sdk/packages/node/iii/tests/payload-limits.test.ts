import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// Capture all `new WebSocket(...)` constructor calls so tests can assert on
// the options the SDK plumbs through. `vi.hoisted` lets the spy survive the
// hoisting of `vi.mock` and stay reachable from inside the factory.
const wsConstructorSpy = vi.hoisted(() => vi.fn())

vi.mock('ws', () => {
  // Minimal WebSocket-like stand-in. We only need the API surface the SDK
  // touches before the connection is "open": `on`, `removeAllListeners`,
  // `terminate`, `send`, `close`, plus the static `OPEN` constant.
  class FakeWebSocket {
    public readyState = 0
    public listeners: Record<string, Array<(...args: unknown[]) => void>> = {}
    static OPEN = 1
    static CONNECTING = 0
    static CLOSING = 2
    static CLOSED = 3

    constructor(address: string, options?: unknown) {
      wsConstructorSpy(address, options)
    }

    on(event: string, fn: (...args: unknown[]) => void): this {
      this.listeners[event] ??= []
      this.listeners[event].push(fn)
      return this
    }

    removeAllListeners(): this {
      this.listeners = {}
      return this
    }

    terminate(): void {}
    close(): void {}
    send(_data: string, cb?: (err?: Error) => void): void {
      cb?.()
    }
  }

  return { WebSocket: FakeWebSocket, default: FakeWebSocket }
})

// Stub the OTel system so importing the SDK doesn't try to wire up a real
// exporter — `initOtel` is called from the Sdk constructor.
vi.mock('../src/telemetry-system', () => {
  return {
    extractContext: () => ({}),
    getLogger: () => undefined,
    getMeter: () => undefined,
    getTracer: () => undefined,
    initOtel: () => {},
    injectBaggage: () => undefined,
    injectTraceparent: () => undefined,
    SeverityNumber: { ERROR: 17 },
    shutdownOtel: async () => {},
    SpanKind: { SERVER: 1 },
    withSpan: async (_name: string, _opts: unknown, fn: () => unknown) => await fn(),
  }
})

const DEFAULT_LIMIT = 16 * 1024 * 1024
const ADDRESS = 'ws://localhost:65535'

// Dynamic import so we pick up the SDK with the mocked `ws` module — the
// shared `setupFiles` already loaded the real SDK once, so we have to reset
// modules before re-importing.
async function loadSdk(): Promise<typeof import('../src/iii') & typeof import('../src/errors')> {
  vi.resetModules()
  const iii = await import('../src/iii')
  const errors = await import('../src/errors')
  return { ...iii, ...errors }
}

describe('Node SDK payload limits', () => {
  beforeEach(() => {
    wsConstructorSpy.mockClear()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('plumbs init option maxMessageSize to ws maxPayload', async () => {
    const { registerWorker } = await loadSdk()
    const limit = 8 * 1024 * 1024
    const sdk = registerWorker(ADDRESS, { maxMessageSize: limit })

    expect(wsConstructorSpy).toHaveBeenCalledTimes(1)
    const [address, options] = wsConstructorSpy.mock.calls[0]
    expect(address).toBe(ADDRESS)
    expect((options as { maxPayload?: number }).maxPayload).toBe(limit)

    await sdk.shutdown()
  })

  it('defaults maxMessageSize to 16 MiB when not supplied', async () => {
    const { registerWorker } = await loadSdk()
    const sdk = registerWorker(ADDRESS)

    expect(wsConstructorSpy).toHaveBeenCalledTimes(1)
    const [, options] = wsConstructorSpy.mock.calls[0]
    expect((options as { maxPayload?: number }).maxPayload).toBe(DEFAULT_LIMIT)

    await sdk.shutdown()
  })

  it('throws IIIPayloadTooLarge when trigger payload exceeds the limit', async () => {
    const { registerWorker, IIIPayloadTooLarge } = await loadSdk()
    const limit = 4 * 1024
    const sdk = registerWorker(ADDRESS, { maxMessageSize: limit })

    // Build a payload that obviously exceeds the limit once JSON-encoded.
    const oversize = 'A'.repeat(limit * 2)

    await expect(
      sdk.trigger({ function_id: 'noop', payload: { blob: oversize } }),
    ).rejects.toBeInstanceOf(IIIPayloadTooLarge)

    try {
      await sdk.trigger({ function_id: 'noop', payload: { blob: oversize } })
      expect.fail('expected oversize trigger to throw')
    } catch (err) {
      expect(err).toBeInstanceOf(IIIPayloadTooLarge)
      const message = (err as Error).message
      expect(message).toContain(`limit ${limit}`)
      expect(message).toContain('channels')
      expect(message).toContain('https://iii.dev/docs/how-to/use-channels')
    }

    await sdk.shutdown()
  })

  it('does not throw when the serialized payload is within the limit', async () => {
    const { registerWorker, IIIPayloadTooLarge } = await loadSdk()
    const limit = 1024 * 1024
    const sdk = registerWorker(ADDRESS, {
      maxMessageSize: limit,
      // Avoid waiting the full default timeout when the WS never opens.
      invocationTimeoutMs: 50,
    })

    // Trigger should not throw IIIPayloadTooLarge synchronously. The promise
    // will reject later with TIMEOUT (no real WS), which is fine — we only
    // care that the producer guard let it through.
    const promise = sdk.trigger({ function_id: 'noop', payload: { hello: 'world' } })

    await expect(promise).rejects.not.toBeInstanceOf(IIIPayloadTooLarge)

    await sdk.shutdown()
  })
})

describe.skipIf(!process.env.III_URL)('Node SDK payload limits — integration', () => {
  it('engine returns invocation_failed_payload_too_large for oversize trigger', async () => {
    // Use the real SDK from the shared setup (skipping our mock) by importing
    // through `vi.importActual` — the engine on `III_URL` will close the WS
    // when it sees the oversize message and we want a real round-trip.
    const { registerWorker } = await vi.importActual<typeof import('../src/index')>('../src/index')
    const sdk = registerWorker(process.env.III_URL as string, {
      maxMessageSize: 32 * 1024 * 1024,
    })

    // 20 MiB raw blob — should hit the engine ceiling and come back with the
    // specific error code (Phase 1 contract).
    const payload = { blob: 'A'.repeat(20 * 1024 * 1024) }

    try {
      await sdk.trigger({ function_id: 'noop', payload })
      expect.fail('expected oversize trigger to reject')
    } catch (err) {
      const code = (err as { code?: string }).code
      expect(code).toBe('invocation_failed_payload_too_large')
    } finally {
      await sdk.shutdown()
    }
  })
})
