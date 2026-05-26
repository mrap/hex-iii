---
type: how-to
title: Run an iii worker in this instance
---

# When to use

You've authored a worker per [`iii://iii/authoring`](iii://iii/authoring)
(or you're about to) and now need to actually start the process so its
WebSocket opens against this engine. This page covers the deployment
paths available to the harness in this iii instance — file the source
through [`coder::*`](iii://coder/coder), boot it through
[`sandbox::*`](iii://sandbox), verify it joined the bus through
[`directory::engine::*`](iii://directory/engine/workers/list).

For pre-built workers published to the public registry,
[`worker::add`](iii://worker/add) is faster — see Path B below
before reaching for Path A.

# Step 0 — discover what's already there

Always run this before authoring a new worker. The capability you
need may already be one call away:

- `directory::engine::functions::list` — every function currently registered with this engine, across every connected worker.
- `directory::registry::workers::list` — every worker published in the public registry. If your target capability is here, prefer Path B and skip authoring entirely.

Only continue with Path A when both come up empty.

# Path A — ephemeral sandbox (harness default)

The harness has [`coder::*`](iii://coder/coder) for writing files and
[`sandbox::*`](iii://sandbox) for booting Node microVMs. Combine them
to run a worker without touching the host or going through the
worker-ops daemon. The sandbox is ephemeral — when it stops, the
worker disconnects and its functions evaporate from the engine.

## 1. Write the worker source

```jsonc
// coder::create-file
{
  "files": [{
    "path": "workers/github.ts",
    "content": "import { registerWorker } from 'iii-sdk'\nconst iii = registerWorker(process.env.III_URL!, { workerName: 'github' })\niii.registerFunction('github::ping', async () => ({ ok: true }))\n",
    "parents": true
  }]
}
```

Write a `package.json` next to it so `npm install` works:

```jsonc
// coder::create-file
{
  "files": [{
    "path": "workers/package.json",
    "content": "{\n  \"name\": \"agent-worker\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"dependencies\": { \"iii-sdk\": \"latest\" }\n}\n"
  }]
}
```

## 2. Boot a Node sandbox

```jsonc
// sandbox::create
{ "image": "node" }
// → { "sandbox_id": "550e8400-..." }
```

The `node` image is in this engine's catalog
(see [`config.yaml`](config.yaml) line 58-61). `auto_install: true`
means the daemon pulls it on first use if the cache is cold.

## 3. Install dependencies inside the sandbox

```jsonc
// sandbox::exec
{
  "sandbox_id": "550e8400-...",
  "cmd": "sh",
  "args": ["-c", "cd /workspace/workers && npm install"],
  "timeout_ms": 120000
}
```

You'll need to get the source into the sandbox first. The simplest
path is to `coder::create-file` straight into the sandbox's
`/workspace` mount; alternatively use `sandbox::exec` with `cat <<EOF`
to splat the source from a single command.

## 4. Run the worker

```jsonc
// sandbox::exec
{
  "sandbox_id": "550e8400-...",
  "cmd": "sh",
  "args": ["-c", "cd /workspace/workers && node --experimental-strip-types github.ts"],
  "env": ["III_URL=ws://host.docker.internal:49134"],
  "timeout_ms": 1800000
}
```

**Important gotcha — `sandbox::exec` is blocking and serialized per
`sandbox_id`.** The call will not return while the worker is running.
For a persistent worker, pass a long `timeout_ms` (30 min above) and
let the harness move on to other work — the function registration is
**live the instant the WebSocket handshake completes**, even though
the `sandbox::exec` call is still open. The harness's next call to
`iii.trigger({ function_id: 'github::ping' })` will succeed before the
exec returns.

To run a second command (e.g. install another package, inspect a log)
on the same sandbox while the worker is running, you have two
options: boot a second sandbox, or stop the worker first
(`sandbox::stop` evaporates the whole VM along with the worker).

## 5. Tear it down when you're done

```jsonc
// sandbox::stop
{ "sandbox_id": "550e8400-..." }
```

The engine sees the worker disconnect and removes its functions from
the registry automatically. Any in-flight invocation against the
disappearing worker fails with `invocation_stopped`.

# Path B — registry-managed (when the worker exists publicly)

If `directory::registry::workers::list` shows a worker that already
does what you need:

```jsonc
// worker::add
{
  "source": { "kind": "registry", "name": "image-resize", "version": "0.1.2" },
  "wait":   true
}
```

The worker-ops daemon downloads the artifact under
`~/.iii/managed/{name}/`, writes the entry to `iii.config.yaml`,
pins the resolved version in `iii.lock`, and (with `wait: true`)
blocks until the worker reports ready. See
[`iii://worker/add`](iii://worker/add) for the full surface.

`{ "kind": "local", "path": "..." }` is the third source variant but
it's **CLI-only** — it returns `W102` over the trigger surface so
the harness cannot side-load arbitrary local code through `worker::add`.
For harness-authored ad-hoc workers, use Path A instead.

# Verify the worker came up

After Path A or Path B, three calls confirm the worker is on the bus
and the functions are routable:

```jsonc
// directory::engine::workers::list
// → look for your workerName in the response
```

```jsonc
// directory::engine::functions::list
// → look for your function id in the response (filter with `prefix: "github::"`)
```

```jsonc
// (from anywhere on the bus, including the harness)
// iii.trigger({ function_id: 'github::ping', payload: {} })
// → { ok: true }
```

If the worker doesn't show up:

- Check `sandbox::exec`'s stderr in its response — most common cause is `iii-sdk` install failing or the `III_URL` env var pointing at the wrong host.
- Confirm the engine is on `ws://localhost:49134` from the harness's perspective and `ws://host.docker.internal:49134` from inside the sandbox.
- `directory::engine::workers::list` only shows currently-connected workers; if the worker crashed on startup it never made it onto the list.

# Related

- [`iii://iii/authoring`](iii://iii/authoring) — what to put in the source file before booting it.
- [`iii://iii/recipes`](iii://iii/recipes) — three worked examples that go end-to-end through Path A.
- [`iii://coder/coder`](iii://coder/coder) — the file-writing surface used in step 1.
- [`iii://sandbox/create`](iii://sandbox/create) / [`iii://sandbox/exec`](iii://sandbox/exec) — the sandbox lifecycle.
- [`iii://worker/add`](iii://worker/add) — Path B's full input/output shape.
- [`iii://directory/engine/workers/list`](iii://directory/engine/workers/list) / [`iii://directory/engine/functions/list`](iii://directory/engine/functions/list) — verification surfaces.
