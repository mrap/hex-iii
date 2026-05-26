---
type: index
title: iii
---

# iii

iii is a WebSocket-routed worker mesh. The engine is a single coordinator
process that holds a live registry of every connected worker, every
function those workers expose, and every trigger bound to them. Workers
are independent OS processes that open a WebSocket to the engine and
register Functions (`service::name` handlers) and Triggers (the events
that invoke those Functions). There is no direct worker-to-worker
traffic — every call is routed through the engine, which makes
language, runtime, and physical location of any given worker
invisible to its callers.

**You can extend yourself by writing iii workers.** When the harness
needs a capability no current function provides, the path is:
write a TypeScript file with `coder::create-file`, boot it inside an
`iii-sandbox` Node VM, and the new process opens a WebSocket to the
same engine the harness is already talking to. From the instant that
handshake completes, the new function is callable through `iii.trigger`
exactly like any built-in one. No registration with the harness, no
restart, no polling — the engine routes it automatically.

## The four primitives

- **Engine** — single WebSocket coordinator (port 49134 by default); holds the live registry; routes every invocation.
- **Worker** — process that opens a WebSocket to the engine and registers things. Can be anywhere a network can reach the engine. Can be ephemeral (one-shot) or persistent.
- **Function** — `service::name` handler inside a worker. Stable across worker restarts because the engine re-routes by id, not by address.
- **Trigger** — `(type, config, function_id)` triple that causes a Function to run. Types include built-ins like HTTP, cron, queue, plus any custom type a worker publishes.

## Smallest possible self-extension

Three lines of TypeScript get a new function on the bus:

```ts
import { registerWorker } from 'iii-sdk'
const iii = registerWorker(process.env.III_URL!, { workerName: 'demo' })
iii.registerFunction('demo::add', async ({ a, b }: { a: number; b: number }) => ({ c: a + b }))
```

The instant the WebSocket handshake finishes, the harness (or anything
else on the bus) can invoke `demo::add` via
`iii.trigger({ function_id: 'demo::add', payload: { a: 2, b: 3 } })`.

## How-tos

- [`iii://iii/primitives`](iii://iii/primitives) — engine, worker, function, trigger; states, decoupling rules, topology diagram. Read first if the mental model isn't sharp yet.
- [`iii://iii/authoring`](iii://iii/authoring) — full TypeScript SDK surface: `registerWorker`, `registerFunction`, `trigger`, `registerTrigger`, `registerTriggerType`, errors, schemas, lifecycle.
- [`iii://iii/run-it`](iii://iii/run-it) — how to actually boot a harness-authored worker in this iii instance via `sandbox::*`, plus how to verify it joined the bus with `directory::engine::workers::list`.
- [`iii://iii/recipes`](iii://iii/recipes) — three worked end-to-end examples: integrating a third-party service, publishing a custom reactive trigger type, running a one-shot ephemeral worker.

## Before writing a worker

Check whether the capability already exists. Workers in the public
registry can usually be installed with one call instead of authored
from scratch:

- `directory::engine::functions::list` — every function currently registered with this engine.
- `directory::registry::workers::list` — every worker published in `api.workers.iii.dev`. If the registry has it, prefer `worker::add { source: { kind: "registry", name: "..." } }` over re-implementing it.

Only reach for hand-authored workers when the gap is real.
