# sandbox — ephemeral VMs in this engine

The `iii-sandbox` daemon owns the `sandbox::*` triggers. Each sandbox is scoped to the engine it runs inside. Use them for running an LLM-generated patch against an untrusted codebase, ephemeral build workers, or integration test fixtures.

## Operations

- [`sandbox/create`](iii://sandbox/create) — spin up a sandbox from a catalog image
- [`sandbox/exec`](iii://sandbox/exec) — run a command inside a sandbox
- [`sandbox/stop`](iii://sandbox/stop) — tear a sandbox down
- [`sandbox/list`](iii://sandbox/list) — list running sandboxes

Live data: [`iii://fn/sandbox/list`](iii://fn/sandbox/list) returns the current roster inline.

## Image catalog

`sandbox/create` rejects any image not in the daemon's catalog (presets `python` / `node`, plus operator-added custom images in `iii.config.yaml`). An empty catalog with no presets denies every call — a deliberate fail-closed default.

## Error envelope

Every op returns errors as `{ "type": "<category>", "code": "Sxxx", "message": "...", "docs_url": "...", "retryable": bool }`. `type` is the category (`validation`, `config`, `internal`, `transient`, `execution`, `filesystem`, `platform`) — not the literal string `"SandboxError"`. The `S` codes are independent of the `W` codes used by `worker::*`.
