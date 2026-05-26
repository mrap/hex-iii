---
type: how-to
title: Extend the harness by writing a worker
---

# When to use

The harness needs a capability — call a third-party API, react to
an external event, run a heavy one-shot job — and no current
function on the bus provides it. This page is the decision tree plus
three end-to-end worked examples that put the surface from
[`iii://iii/authoring`](iii://iii/authoring) and the deploy steps
from [`iii://iii/run-it`](iii://iii/run-it) together.

# Decision tree

Ask these three questions in order. Stop at the first "yes".

1. **Does the function already exist on the bus?**
   Check `directory::engine::functions::list`. If yes, just call it
   with `iii.trigger`. Done.

2. **Is the worker published in the public registry?**
   Check `directory::registry::workers::list`. If yes, install it
   with [`worker::add { source: { kind: "registry", name: "..." } }`](iii://worker/add).
   Done.

3. **Otherwise — write a worker.** This is the path below.

The "write a worker" path is for genuine gaps. Don't reach for it
when an existing function would do; the harness has more leverage
calling iii functions than re-implementing them.

# Recipe 1 — Integrate a third-party service

**Scenario.** The harness needs to create GitHub issues. No
`github::*` functions are registered, no `github-worker` is in the
registry. You write a worker that wraps `@octokit/rest` and exposes
`github::create-issue` as an iii function.

## 1. Write the source

```ts
// workers/github/worker.ts
import { registerWorker } from 'iii-sdk'
import { Octokit } from '@octokit/rest'

const iii = registerWorker(process.env.III_URL!, { workerName: 'github' })
const gh = new Octokit({ auth: process.env.GITHUB_TOKEN })

iii.registerFunction(
  'github::create-issue',
  async (payload: { owner: string; repo: string; title: string; body?: string }) => {
    const { data } = await gh.issues.create({
      owner: payload.owner,
      repo:  payload.repo,
      title: payload.title,
      body:  payload.body,
    })
    return { number: data.number, url: data.html_url }
  },
  {
    description: 'Create a GitHub issue in {owner}/{repo}.',
    request_format: {
      type: 'object',
      properties: {
        owner: { type: 'string' },
        repo:  { type: 'string' },
        title: { type: 'string' },
        body:  { type: 'string' },
      },
      required: ['owner', 'repo', 'title'],
    },
    response_format: {
      type: 'object',
      properties: {
        number: { type: 'integer' },
        url:    { type: 'string' },
      },
      required: ['number', 'url'],
    },
  },
)

process.on('SIGTERM', () => iii.shutdown().then(() => process.exit(0)))
```

`package.json` next to it declares `iii-sdk` and `@octokit/rest`.

## 2. Boot

Follow [`iii://iii/run-it`](iii://iii/run-it) Path A: `sandbox::create
{ image: "node" }`, drop the source in, `npm install`, then
`sandbox::exec` to run `node worker.ts` with `III_URL` and
`GITHUB_TOKEN` in the env list. Use a long `timeout_ms` so the worker
stays up.

## 3. Use it from the harness

```jsonc
// from any other function on the bus, including the harness:
// iii.trigger({
//   function_id: 'github::create-issue',
//   payload: { owner: 'iii-hq', repo: 'workers', title: 'Bug: ...' }
// })
// → { number: 1234, url: 'https://github.com/iii-hq/workers/issues/1234' }
```

The harness invokes `github::create-issue` exactly like a built-in
function. The fact that you authored it ten seconds ago is invisible
at the call site.

## When to generalise

If you find yourself writing similar wrappers repeatedly, publish
the worker to the registry so future harness sessions hit Path B
instead of Path A. The hand-authored sandbox path is for gaps that
aren't worth packaging.

# Recipe 2 — Publish a new reactive trigger type (no polling)

**Scenario.** You want other functions to run automatically when a
file under a watched directory changes. The built-in trigger types
(`webhook`, `cron`, `queue`, `state-change`) don't cover filesystem
events. You author a `fs::watch` trigger type so any function on the
bus can bind to it.

## 1. Write the source

```ts
// workers/fs-watch/worker.ts
import { registerWorker, TriggerAction } from 'iii-sdk'
import chokidar from 'chokidar'

const iii = registerWorker(process.env.III_URL!, { workerName: 'fs-watch' })

type FsWatchConfig = { path: string; recursive?: boolean }

const bindings = new Map<
  string,
  { function_id: string; config: FsWatchConfig; watcher: chokidar.FSWatcher }
>()

iii.registerTriggerType<FsWatchConfig>(
  { id: 'fs::watch', description: 'Fires when a file under `path` changes.' },
  {
    async registerTrigger({ id, function_id, config }) {
      const watcher = chokidar.watch(config.path, {
        depth: config.recursive === false ? 0 : undefined,
        ignoreInitial: true,
      })
      watcher.on('all', (event, eventPath) => {
        iii.trigger({
          function_id,
          payload: { event, path: eventPath, triggerId: id },
          action: TriggerAction.Void(),
        })
      })
      bindings.set(id, { function_id, config, watcher })
    },
    async unregisterTrigger({ id }) {
      const binding = bindings.get(id)
      if (binding) {
        await binding.watcher.close()
        bindings.delete(id)
      }
    },
  },
)

process.on('SIGTERM', () => iii.shutdown().then(() => process.exit(0)))
```

## 2. Boot

Path A again. `npm install iii-sdk chokidar`, run `node worker.ts`.

## 3. Bind a function to the new trigger type

From any worker — including a second harness-authored worker, or
the harness itself if it's running its own SDK session — bind a
function to `fs::watch`:

```ts
iii.registerTrigger({
  type: 'fs::watch',
  function_id: 'indexer::reindex',
  config: { path: '/data/docs', recursive: true },
})
```

The `fs-watch` worker's `registerTrigger` handler runs, spins up a
chokidar watcher for `/data/docs`, and from that moment any change
under that path fires `indexer::reindex` through `iii.trigger`. No
polling, no scheduled jobs, no glue.

This is the deepest leverage in iii: one worker turns its native
event source into something the entire bus can react to.

# Recipe 3 — One-shot ephemeral worker

**Scenario.** You need to run a single batch job (e.g. download a
dataset, transform it, upload the result) and you want its progress
exposed as an iii function so the harness or another worker can poll
status without you reinventing IPC.

## Write and run as one process

```ts
// workers/etl/worker.ts
import { registerWorker } from 'iii-sdk'

const iii = registerWorker(process.env.III_URL!, { workerName: 'etl-2026-05' })

let state: { phase: 'pending' | 'extracting' | 'transforming' | 'loading' | 'done'; rows: number } = {
  phase: 'pending',
  rows:  0,
}

iii.registerFunction('etl-2026-05::status', async () => state)

;(async () => {
  state = { phase: 'extracting', rows: 0 }
  const rows = await extract()
  state = { phase: 'transforming', rows: rows.length }
  const out = await transform(rows)
  state = { phase: 'loading', rows: out.length }
  await load(out)
  state = { phase: 'done', rows: out.length }
  await iii.shutdown()
  process.exit(0)
})()
```

The worker registers `etl-2026-05::status`, runs the actual job
inline, mutates `state` as it progresses, and calls
`iii.shutdown()` when it finishes. The engine evicts the worker's
function from the registry on disconnect — `etl-2026-05::status`
disappears as soon as the work is done, which is the correct
post-condition.

While the job runs, anyone on the bus can poll:

```jsonc
// iii.trigger({ function_id: 'etl-2026-05::status', payload: {} })
// → { phase: 'transforming', rows: 8421 }
```

A subtle win: the `workerName` includes the job's identity
(`etl-2026-05`) so multiple ETL runs can coexist without naming
collisions, and the function id is scoped to the same identity for
the same reason.

## When NOT to use this pattern

If the job is long-lived and you'd want it to survive a restart, use
[`worker::add`](iii://worker/add) with a registered worker instead —
the worker-ops daemon will respawn the process on failure. Recipe 3
is for genuinely one-off work.

# Anti-patterns

- **Polling instead of trigger types.** If you find yourself writing a function that runs on a cron and reads a queue/database/file every N seconds, the event source can probably be wrapped as a custom trigger type. Recipe 2 is the model.
- **Reinventing existing workers.** Always run step 1 of the decision tree (`directory::engine::functions::list`) and (`directory::registry::workers::list`) before writing a wrapper. The harness's most common mistake is to wrap something that already exists.
- **Side-channel state between workers.** Use [`iii-state`](https://workers.iii.dev/workers/iii-state.md) (the `state::*` functions) for shared key/value state. Don't write workers that read each other's files or hit each other's HTTP endpoints — every cross-worker call should go through `iii.trigger`.

# Related

- [`iii://iii/primitives`](iii://iii/primitives) — the four pieces these recipes compose.
- [`iii://iii/authoring`](iii://iii/authoring) — the SDK calls behind every snippet above.
- [`iii://iii/run-it`](iii://iii/run-it) — the deployment steps each recipe skims past.
- [`iii://directory/registry/workers/list`](iii://directory/registry/workers/list) — step 2 of the decision tree.
