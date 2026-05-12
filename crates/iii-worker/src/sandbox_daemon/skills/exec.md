# sandbox/exec — run a command in a sandbox

Execute a command in a running sandbox and stream stdout/stderr back. Per-sandbox serialization: only one exec runs at a time — a concurrent call against the same `sandbox_id` returns **S003**. The call returns when the child exits, the timeout fires, or the VM becomes unreachable.

- id: `sandbox::exec`
- timeout: caller-set (pass `timeout_ms`)
- idempotent: no
- request: `ExecRequest { sandbox_id, cmd, args?, stdin?, env?, timeout_ms?, workdir? }`
- response: `ExecResponse { stdout, stderr, exit_code?, timed_out, duration_ms, success }`

- `cmd` is the binary as a single string, NOT an argv array.
- `args` is the argv tail (`Vec<String>`).
- `stdin` is base64-encoded bytes.
- `env` is `Vec<String>` of `"K=V"` entries.

## Example

```json
{
  "sandbox_id": "550e8400-e29b-41d4-a716-446655440000",
  "cmd": "bash",
  "args": ["-lc", "echo hello && date"],
  "env": ["FOO=bar"],
  "workdir": "/workspace",
  "timeout_ms": 30000
}
```

Response:
```json
{
  "stdout": "hello\nMon Oct 13 19:42:11 UTC 2025\n",
  "stderr": "",
  "exit_code": 0,
  "timed_out": false,
  "duration_ms": 42,
  "success": true
}
```

`success` is `true` iff `exit_code == 0` and `timed_out == false`. `exit_code: 127` means "command not found"; `126` means "not executable". Per POSIX, spawn failures surface in `exit_code`, NOT as an error envelope.

## Errors

- **S001** invalid request (bad `sandbox_id` UUID, missing `cmd`).
- **S002** sandbox not found.
- **S003** concurrent exec — await the previous one first.
- **S200** exec timed out (stdout/stderr captured pre-timeout are still returned).
- **S300** VM unreachable (boot failed earlier, or shell socket dropped).
