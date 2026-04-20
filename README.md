![iii banner image](website/public/og-image.png)

[![Engine License](https://img.shields.io/badge/engine-ELv2-blue.svg)](engine/LICENSE)
[![SDK License](https://img.shields.io/badge/sdk-Apache--2.0-green.svg)](sdk/LICENSE)
[![Spec](https://img.shields.io/badge/spec-draft-orange.svg)](spec/)
[![Docker](https://img.shields.io/docker/v/iiidev/iii?label=docker)](https://hub.docker.com/r/iiidev/iii)
[![npm](https://img.shields.io/npm/v/iii-sdk?label=npm)](https://www.npmjs.com/package/iii-sdk)
[![PyPI](https://img.shields.io/pypi/v/iii-sdk?label=pypi)](https://pypi.org/project/iii-sdk/)
[![Crates.io](https://img.shields.io/crates/v/iii-sdk?label=crates.io)](https://crates.io/crates/iii-sdk)

## What iii is

**iii is one protocol for anything that does work.**

Modern systems are made of many different things that all need to call each other: services, third‑party APIs, queues, schedulers, agents, browsers, sandboxes, devices, scripts, jobs. Each one comes with its own way of being invoked, observed, scheduled, and connected. The result is a permanent integration tax — half your code, half your operations, and half your debugging is the seams between things, not the things themselves.

iii collapses that with a small, complete model:

- **Function** — anything callable.
- **Trigger** — anything that runs a Function.
- **Worker** — anything that hosts Functions or emits Triggers.

Three nouns. One protocol. Any language, any location, any runtime.

Every capability — your services, your agents, your browsers, your devices, your third‑party APIs, your schedulers, your sandboxes — speaks the same protocol and shows up in the same registry. They become callable, composable, observable, and discoverable as if they were one system.

## Why this matters

iii is not a "backend stack." It is the missing **execution protocol** that lets anything that runs work with anything else that runs, regardless of language, location, or trust boundary.

Once a thing speaks iii:

- Any language can call it. Any language can implement it.
- It can be triggered by HTTP, cron, queues, streams, state changes, or any custom event source — without that source having to know what runs in response.
- Its calls, retries, failures, and traces flow through one observability pipeline.
- It becomes part of a live, discoverable graph. New capabilities appear in real time without redeploys.
- It can be wrapped around things you already have. Existing HTTP endpoints become first‑class Functions without rewrites.

The boundaries between services, agents, browsers, devices, and APIs collapse. You stop integrating systems and start composing one.

## The Three Primitives

| Primitive    | What it is                                                                                                                                                                              |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Function** | A unit of work with a typed input and optional output. Implemented by a Worker, fronted by an HTTP endpoint, or wrapped around something that already exists. Identified globally.      |
| **Trigger**  | Anything that causes a Function to run — an HTTP route, a cron schedule, a queue message, a state change, a stream event, or a custom event source contributed by a Worker.             |
| **Worker**   | Anything that connects to iii to host Functions, emit Triggers, or contribute capabilities. A Worker can be a service, an agent, a browser tab, a device, a CLI, or a third‑party API.  |

A small set of capabilities (queue, observability, HTTP entry) live inside the engine because they are load‑bearing — they shape what the primitives guarantee. Everything else is a Worker, and over time even those capabilities will be pluggable behind stable interfaces.

## The Spec

iii is, at its core, a wire‑level protocol that anyone can implement. The protocol is intentionally separated from the engine so that other engines, other SDKs, and other transports can speak it.

- [`spec/`](spec/) — protocol specification, wire formats, transport bindings, and capability interfaces.
- [`spec/core-protocol.md`](spec/core-protocol.md) — the abstract core protocol (transport‑agnostic).

The spec covers the message model, lifecycle, identity, observability requirements, error model, and conformance. Wire formats (JSON today, others possible) and transport bindings (WebSocket, HTTP, gRPC) are companion documents.

> **Spec license:** to be determined. The spec lives in this repo so it can evolve with the reference implementation, but its final license terms are still being decided.

## Quick Start

Get started with iii by following the [Quickstart guide](https://iii.dev/docs/quickstart).

## SDKs

| Language | Package                                            | Install               |
| -------- | -------------------------------------------------- | --------------------- |
| Node.js  | [`iii-sdk`](https://www.npmjs.com/package/iii-sdk) | `npm install iii-sdk` |
| Python   | [`iii-sdk`](https://pypi.org/project/iii-sdk/)     | `pip install iii-sdk` |
| Rust     | [`iii-sdk`](https://crates.io/crates/iii-sdk)      | Add to `Cargo.toml`   |

The SDKs are reference implementations of the protocol. New SDKs in any language can be built directly against the [spec](spec/).

## Agent Skills

Give your AI coding agent full context on iii:

```bash
npx skillkit add iii-hq/iii/skills
```

Skills covering every iii primitive — HTTP endpoints, queues, cron, state, streams, custom triggers, and more. Works with Claude Code, Cursor, Gemini CLI, Codex, and [30+ other agents](https://agentskills.io). See [skills/](skills/) for the full list.

## Console

The [iii-console](console/) is a developer and operations dashboard for inspecting Functions, Triggers, Workers, traces, and real‑time state. See the [Console docs](https://iii.dev/docs/console) for setup and usage.

## Repository Structure

| Directory     | What it is                                                | README                                 |
| ------------- | --------------------------------------------------------- | -------------------------------------- |
| `spec/`       | Protocol specification (transport‑ and language‑agnostic) | [spec/README.md](spec/README.md)       |
| `engine/`     | iii Engine (Rust) — reference implementation              | [engine/README.md](engine/README.md)   |
| `sdk/`        | SDKs for Node.js, Python, and Rust                        | [sdk/README.md](sdk/README.md)         |
| `console/`    | Developer dashboard (React + Rust)                        | [console/README.md](console/README.md) |
| `frameworks/` | Higher‑level frameworks built on the SDK                  | [frameworks/motia/](frameworks/motia/) |
| `skills/`     | Agent skills for AI coding agents                         | [skills/README.md](skills/README.md)   |
| `website/`    | iii.dev marketing site                                    | [website/](website/)                   |
| `docs/`       | Documentation site (Mintlify/MDX)                         | [docs/README.md](docs/README.md)       |

See [STRUCTURE.md](STRUCTURE.md) for the full monorepo layout, dependency chain, and CI/CD details.

## Resources

- [Documentation](https://iii.dev/docs)
- [Spec](spec/)
- [Examples](https://github.com/iii-hq/iii-examples)
- [Console](console/)
- [Contributing](CONTRIBUTING.md)

## License

iii is licensed across three layers:

| Layer                           | License                                        |
| ------------------------------- | ---------------------------------------------- |
| `spec/` — protocol              | **TBD** (see [`spec/README.md`](spec/README.md)) |
| `engine/` — reference engine    | [Elastic License 2.0](engine/LICENSE)          |
| Everything else (SDKs, console, frameworks, docs, website, skills) | [Apache License 2.0](sdk/LICENSE) |

The engine runtime is licensed under the Elastic License 2.0 (ELv2). All SDKs, frameworks, CLI, console, documentation, and the website are licensed under the Apache License 2.0. The protocol specification's final license is being decided.

See [CONTRIBUTING.md](CONTRIBUTING.md) for additional details.
