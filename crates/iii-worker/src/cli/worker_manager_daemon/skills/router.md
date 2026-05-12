# worker — install and manage workers

Owned by the `iii-worker-ops` daemon (auto-spawned as an engine sidecar). Every op below is also callable as `iii worker <cmd>` on the CLI; the trigger surface is the SDK path.

## Operations

- [`worker/add`](iii://worker/add) — install from registry or OCI reference
- [`worker/remove`](iii://worker/remove) — uninstall (consent required)
- [`worker/update`](iii://worker/update) — re-resolve registry versions
- [`worker/start`](iii://worker/start) — start a configured worker
- [`worker/stop`](iii://worker/stop) — stop a running worker (consent required)
- [`worker/list`](iii://worker/list) — installed + run state + versions
- [`worker/clear`](iii://worker/clear) — wipe cached artifacts (consent required)
- [`worker/schema`](iii://worker/schema) — discover request/response shapes

Live data: [`iii://fn/worker/schema`](iii://fn/worker/schema) returns the full JSON Schema for all 8 ops, plus per-op `default_timeout_ms` and `idempotent` hints.

## Error envelope

All ops return errors as `{ "type": "WorkerOpError", "code": "Wxxx", "message": "...", "details": {...} }`.

Codes:
- **W100** InvalidName — name doesn't match `[a-z0-9_-]{1,64}`.
- **W101** InvalidSource — required field missing or malformed.
- **W102** LocalPathNotAllowedViaTrigger — `kind: "local"` is CLI-only.
- **W103** MissingTarget — empty `names` + `all` unset, or both set.
- **W104** ConsentRequired — destructive op needs `yes: true`.
- **W110** NotFound — worker not in registry / not installed.
- **W900** Internal — unexpected failure.
