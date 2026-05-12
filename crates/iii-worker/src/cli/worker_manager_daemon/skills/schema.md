# worker/schema — discover trigger schemas

Introspect the JSON Schemas for every `worker::*` trigger. Each entry carries field descriptions, defaults, types, plus per-op `default_timeout_ms` and `idempotent` hints. Construct payloads from this without source-diving.

- id: `worker::schema`
- timeout: 10s
- idempotent: yes (pure read)
- request: `SchemaRequest { function_id? }`
- response: `SchemaResponse { schemas: [SchemaEntry] }`

## Example

Omit `function_id` to list all 8 ops:
```json
{}
```

Each `SchemaEntry`:
```json
{
  "function_id": "worker::add",
  "description": "Install a worker from registry name or OCI ref",
  "request": { "...": "JSON Schema for AddOptions" },
  "response": { "...": "JSON Schema for AddOutcome" },
  "default_timeout_ms": 600000,
  "idempotent": true
}
```

Also reachable inline as a section URI: [`iii://fn/worker/schema`](iii://fn/worker/schema).
