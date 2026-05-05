# Automotive Host

`automotive-host` is the future core runtime broker of the automotive product.
It supervises vendor diagnostic software such as Op-Com, captures diagnostic
traffic in real time, and exposes typed operations to the `claw` fork without
moving automotive business logic into the fork core.

## What It Is

`automotive-host` is not a generic tool server.
It is a Windows-first, same-machine automotive execution host that owns:

- process supervision for vendor diagnostic software
- UI Automation and Win32 fallback control paths
- **diagnostic traffic interception** — virtual COM / relay, raw capture, parsers
- typed automotive sessions and audit trails
- policy, approvals, and procedures for risky actions
- a thin bridge for `claw` orchestration

## Current Direction

The project is converging from a typed scaffold into a real runtime broker.
The canonical direction is:

- `automotive-host` owns the process tree, COM-port locks, session state, and
  safety logic
- `claw` remains the orchestrator and operator UX shell
- the primary V1 control surface is a persistent local broker protocol over
  stdio JSON-lines
- `.claw/automotive-state.json` is the observability surface
- HTTP is intentionally deferred in V1 to avoid unnecessary attack surface

## Architecture At A Glance

- `supervisor` — process ownership, Job Object, heartbeat, emergency stop
- `host` — request routing and runtime dispatch
- `types` — stable cross-adapter contracts (`DtcRecord`, taxonomy helpers)
- `schemas` — typed request/response envelopes
- `events` — internal automotive event model
- `failure` — typed failure taxonomy and recovery mapping
- `sessions` — session state, locks, checkpoints, approval tickets
- `policy` — zero-trust evaluation rules
- `procedures` — deterministic procedure definitions
- `audit` — append-only evidence and operation trail
- **diagnostic traffic interception** — raw capture, relay, and parser layers
- `ui_automation` — UIA helpers and Win32 fallback
- `adapters` — per-software isolation: Op-Com first, others later
- `bridge` — mapping to `claw` transport/state surfaces

## Product Principles

- Own the process tree.
- Orchestrate vendor tools, do not replace them.
- Capture first, parse second.
- Keep all automotive-specific rules inside `automotive-host`.
- Prefer typed contracts, explicit state machines, and append-only audit.
- Treat dangerous operations as procedure-driven and policy-gated by default.

## Verify

```bash
cd automotive-host
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Documentation

- `ROADMAP-v3.md` — canonical detailed roadmap, ADRs, backlog, and acceptance criteria
- `AGENTS.md` — engineering rules for agents working in this crate
- `DEVELOPING.md` — module boundaries, DTC versioning, branching notes for fork integration
