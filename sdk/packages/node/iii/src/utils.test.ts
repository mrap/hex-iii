import { describe, expect, it, vi } from 'vitest'
import { http, httpStream } from './utils'

function fakeInternal() {
  const sent: string[] = []
  const response = {
    sendMessage: (m: string) => {
      sent.push(m)
    },
    stream: { end: vi.fn() } as any,
    close: vi.fn(),
  } as any
  return {
    sent,
    req: {
      path_params: {},
      query_params: {},
      headers: {},
      method: 'GET',
      path: '/x',
      body: { a: 1 },
      request_body: { read: vi.fn() } as any,
      response,
    } as any,
  }
}

describe('http wrapper', () => {
  it('delivers a buffered HttpRequest (body present, request_body stripped) and returns HttpResponse', async () => {
    const { req } = fakeInternal()
    const cb = vi.fn(async (r: any) => {
      expect(r.body).toEqual({ a: 1 })
      expect(r.request_body).toBeUndefined()
      return { status_code: 200 }
    })
    const result = await http(cb)(req)
    expect(result).toEqual({ status_code: 200 })
  })
})

describe('httpStream wrapper', () => {
  it('delivers a StreamingRequest (request_body present, body stripped)', async () => {
    const { req } = fakeInternal()
    const cb = vi.fn(async (r: any) => {
      expect(r.request_body).toBeDefined()
      expect(r.body).toBeUndefined()
    })
    await httpStream(cb)(req)
    expect(cb).toHaveBeenCalled()
  })
})
