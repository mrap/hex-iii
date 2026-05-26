---
type: how-to
title: Author an iii worker in TypeScript
---

# When to use

Open this when you've decided to write a worker (per
[`iii://iii/recipes`](iii://iii/recipes)) and want the exact SDK
calls. Every snippet is `iii-sdk` (the Node / TypeScript SDK); the
shape is the same in the Python and Rust SDKs, with the names
adapted to each language's idiom.

If you don't yet have the four primitives clear in your head, read
[`iii://iii/primitives`](iii://iii/primitives) first — most of the
methods below will read like arbitrary API calls otherwise.

# Install

```bash
pnpm add iii-sdk
# or: npm install iii-sdk
```

The single import surface is `iii-sdk`. Subpath exports
(`iii-sdk/stream`, `iii-sdk/state`, `iii-sdk/telemetry`) carry the
specialised types for the iii-state, iii-stream, and OpenTelemetry
integrations — ignore them unless you're swapping the engine's
default state / stream backend.

# Connect — `registerWorker`

```ts
import { registerWorker } from 'iii-sdk'

const iii = registerWorker(
  process.env.III_URL ?? 'ws://localhost:49134',
  {
    workerName: 'my-worker',
    invocationTimeoutMs: 30_000,
  },
)
```

`registerWorker(url, options?)` opens the WebSocket to the engine
**synchronously from your code's perspective** — there is no
separate `await connect()` step. The returned `iii` handle exposes
every method below; you can start calling `registerFunction` and
`trigger` immediately and the SDK queues them until the handshake
lands.

Key options (`InitOptions`):

| Field                  | Default                          | Notes                                                                           |
|------------------------|----------------------------------|---------------------------------------------------------------------------------|
| `workerName`           | `hostname:pid`                   | Display name in `directory::engine::workers::list`. Use something recognisable. |
| `invocationTimeoutMs`  | `30000`                          | Default `timeoutMs` for `iii.trigger(...)` calls.                               |
| `reconnectionConfig`   | exponential, infinite retries    | `initialDelayMs`, `backoffMultiplier`, `jitterFactor`, `maxDelayMs`, `maxRetries`. |
| `otel.enabled`         | `true`                           | Disables OpenTelemetry export. Set `false` or `OTEL_ENABLED=false` to silence.  |

`url` is the engine WebSocket. In this environment it's
`ws://localhost:49134` from the host, or
`ws://host.docker.internal:49134` from inside a sandbox. Always
prefer reading it from `process.env.III_URL` so the same code works
in both places.

# Publish a Function — `registerFunction`

```ts
const ref = iii.registerFunction(
  'math::add',
  async (payload: { a: number; b: number }) => {
    return { c: payload.a + payload.b }
  },
  {
    description: 'Sum two numbers',
    request_format: {
      type: 'object',
      properties: { a: { type: 'number' }, b: { type: 'number' } },
      required: ['a', 'b'],
    },
    response_format: {
      type: 'object',
      properties: { c: { type: 'number' } },
      required: ['c'],
    },
  },
)
```

Signature: `registerFunction(functionId, handler, options?)`.

- `functionId` — `service::name`. Stable across worker restarts; callers will use this string forever.
- `handler` — `(payload) => Promise<result>`. The payload is whatever JSON the caller sent in `iii.trigger({ payload })`; the result is whatever JSON-serialisable thing you return.
- `options.description` / `options.request_format` / `options.response_format` — JSON-Schema-shaped metadata. Surfaces in `directory::engine::functions::info` and in the iii console. **Today this is metadata only — the engine does not validate payloads against the schemas at runtime**, so still guard your inputs in the handler.

`registerFunction` returns a `FunctionRef` with `ref.id` (the
string) and `ref.unregister()` (drops the function from the engine
without disconnecting the worker). Call `unregister` to retire
a function while keeping the worker alive; let the worker
disconnect to retire all of them at once.

# Invoke a Function — `iii.trigger`

Three modes. They are not three methods; they are one method whose
behaviour changes with the `action` field.

## Synchronous (default — no `action`)

```ts
import type { TriggerRequest } from 'iii-sdk'

const result = await iii.trigger<{ a: number; b: number }, { c: number }>({
  function_id: 'math::add',
  payload: { a: 2, b: 3 },
  timeoutMs: 5_000,
})

console.log(result.c) // 5
```

The caller blocks until the function returns or `timeoutMs` expires.
Errors thrown inside the handler propagate back as
`IIIInvocationError` you can `try`/`catch`. Use this for any call
where you need the return value to continue.

## Fire-and-forget — `TriggerAction.Void()`

```ts
import { TriggerAction } from 'iii-sdk'

await iii.trigger({
  function_id: 'audit::log-event',
  payload: { actor: 'user_42', verb: 'login' },
  action: TriggerAction.Void(),
})
```

The engine dispatches the invocation and returns immediately with
`null`. No retries. The caller cannot observe whether the function
succeeded. Use this for one-way notifications where you don't care
about the result and don't need retry semantics.

## Enqueue — `TriggerAction.Enqueue({ queue })`

```ts
const receipt = await iii.trigger({
  function_id: 'orders::process',
  payload: { orderId: 'ord_123' },
  action: TriggerAction.Enqueue({ queue: 'orders' }),
})

console.log(receipt.messageReceiptId)
```

The engine writes the invocation into a named queue (managed by the
[`iii-queue`](https://iii.dev/docs/0-11-0/workers/iii-queue.md)
worker), returns a receipt id, and a queue consumer picks it up
asynchronously with retries and dead-letter support. Use this when
the work is slow or unreliable and you want the engine to handle
retry + back-pressure for you.

| Action                | Caller blocks? | Retries? | Returns                  |
|-----------------------|----------------|----------|--------------------------|
| (omitted)             | yes            | no       | the function's result    |
| `TriggerAction.Void`  | no             | no       | `null`                   |
| `TriggerAction.Enqueue` | no           | yes      | `{ messageReceiptId }`   |

# Bind a Function to an event — `registerTrigger`

`registerTrigger` binds one of your functions to an event source
some other worker has already published (HTTP, cron, queue, custom).
You do not implement the event source; you only declare *"when this
type of event fires with this config, run that function"*.

```ts
const httpTrigger = iii.registerTrigger({
  type: 'webhook',
  function_id: 'math::add',
  config: { method: 'POST', path: '/math/add' },
})

const cronTrigger = iii.registerTrigger({
  type: 'cron',
  function_id: 'reports::nightly',
  config: { expression: '0 0 0 * * * *' },
})

httpTrigger.unregister()
```

`type` must match a trigger type that some worker has published with
`registerTriggerType`. The `config` shape is owned by that worker's
docs — for the built-ins:

| Type           | Owner            | Config example                                |
|----------------|------------------|-----------------------------------------------|
| `http   `      | `iii-http`       | `{ method: 'POST', path: '/foo' }`            |
| `cron`         | `iii-cron`       | `{ expression: '0 */5 * * * * *' }`           |

`registerTrigger` returns a `Trigger` handle with `unregister()`.
Triggers also evaporate automatically when the worker disconnects.

# Publish a new event source — `registerTriggerType`

This is the highest-leverage primitive. `registerTriggerType`
publishes a *type* of event source other workers can bind to. Your
worker keeps a table of `{ trigger_id → function_id, config }` in
memory and walks it whenever its underlying source fires.

```ts
type FsWatchConfig = { path: string; recursive?: boolean }

const bindings = new Map<string, { function_id: string; config: FsWatchConfig }>()

const watcher = iii.registerTriggerType<FsWatchConfig>(
  { id: 'fs::watch', description: 'Fires when a file under `path` changes.' },
  {
    async registerTrigger({ id, function_id, config }) {
      bindings.set(id, { function_id, config })
      startWatchingPath(id, config) // your own fs.watch / chokidar setup
    },
    async unregisterTrigger({ id }) {
      stopWatchingPath(id)
      bindings.delete(id)
    },
  },
)
```

When your file watcher detects a change, walk the bindings and
invoke each one through the normal `iii.trigger` surface:

```ts
function onFileChange(triggerId: string, eventPath: string) {
  const binding = bindings.get(triggerId)
  if (!binding) return
  iii.trigger({
    function_id: binding.function_id,
    payload: { path: eventPath, triggerId },
    action: TriggerAction.Void(), // events are usually fire-and-forget
  })
}
```

Everything inside `registerTrigger` / `unregisterTrigger` is your
own bookkeeping — the SDK and engine just route the bind/unbind
notifications to you. From the caller's perspective, your custom
type is indistinguishable from a built-in one.

`registerTriggerType` returns a `TriggerTypeRef` with
`unregister()`, `registerTrigger`, and `registerFunction` convenience
methods.

# Errors

Two kinds of failure inside a handler, and they have different
semantics:

- **Throw / reject** — propagates to the caller as `IIIInvocationError`. The error stack is included in the wire payload. Use this for unexpected failures: bad input the handler can't recover from, downstream service crash, programmer mistakes.
- **Return a structured error value** — the call succeeds, the caller sees the error in the return shape (`{ ok: false, reason: '...' }`). Use this for expected failures: validation, "not found", business rules. Strongly typed, observable in logs as a normal invocation.

The rule of thumb: if a retry might succeed, throw. If a retry will
deterministically fail the same way, return a structured value.

# Schemas (`request_format` / `response_format`)

```ts
iii.registerFunction(
  'users::create',
  async (payload) => { /* ... */ },
  {
    description: 'Create a user account',
    request_format: {
      type: 'object',
      properties: {
        email: { type: 'string', description: 'RFC 5321 address' },
        name:  { type: 'string' },
      },
      required: ['email'],
    },
    response_format: {
      type: 'object',
      properties: {
        id:    { type: 'string' },
        email: { type: 'string' },
      },
      required: ['id', 'email'],
    },
  },
)
```

The schema fields are JSON-Schema-shaped (`type`, `properties`,
`required`, `items`, `description`). They are metadata only — the
engine does **not** reject mismatched payloads at runtime today. They
exist so:

1. The iii console can render request/response shapes.
2. The agent-readable skills surface (`directory::engine::functions::info`) can describe the function to an LLM.
3. Future runtime validation hooks have a place to plug in.

Always declare them anyway — they double as documentation for the
next person calling your function.

# Lifecycle

## Graceful shutdown

```ts
process.on('SIGTERM', async () => {
  await iii.shutdown()
  process.exit(0)
})
```

`iii.shutdown()` flushes pending traffic and closes the WebSocket
cleanly. The engine sees `disconnected` and evicts the worker's
functions and trigger bindings immediately; in-flight invocations
against this worker resolve as `invocation_stopped` to their
callers.

## Unregister a single function or trigger

`ref.unregister()` on a `FunctionRef`, `Trigger`, or `TriggerTypeRef`
removes that one registration without affecting the others. Useful
for hot-swapping a handler or for workers that expose a dynamic set
of functions.

## Disconnects and reconnects

The SDK reconnects automatically with exponential backoff (see
`reconnectionConfig` above). Your registrations are replayed
verbatim on reconnect — you do **not** need to re-register manually.
Callers of your functions, however, see an `invocation_stopped`
error during the disconnect window and need to retry; treat it as a
cancellation, not a transient failure.

## Ephemeral workers

For one-shot work (a batch job, a Kubernetes Job, a serverless
container), you don't need a long-running process. Connect, register
the functions the job needs to expose, do the work, call
`iii.shutdown()`, and exit. The engine cleans up automatically. See
the third worked example in [`iii://iii/recipes`](iii://iii/recipes).

# Related

- [`iii://iii/primitives`](iii://iii/primitives) — the mental model behind every method on this page.
- [`iii://iii/run-it`](iii://iii/run-it) — how to actually start the process you've written so the WebSocket gets opened.
- [`iii://iii/recipes`](iii://iii/recipes) — three end-to-end examples using the methods above.
- [Node.js SDK reference (upstream)](https://iii.dev/docs/0-11-0/api-reference/sdk-node.md) — the exhaustive type list.
