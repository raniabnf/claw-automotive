# Automotive Host Engineering Principles

## Role In The System

- `automotive-host` is the future core of the automotive product.
- The `claw` fork is an orchestrator and operator UX shell, not the owner of automotive business rules.
- Automotive domain logic, safety policy, approval workflow, procedures, and audit semantics must stay in `automotive-host`.
- The bridge from `claw` into `automotive-host` must stay thin, stateless, and replaceable.

## Development Strategy

- Build `automotive-host` as an independent product first.
- Consume upstream `claw-code` changes as long as our integration still uses extension seams instead of deep core patches.
- Prefer a persistent local broker over stdio first; if the one-shot plugin lifecycle proves too weak for long-lived sessions, move to named pipes without changing the protocol envelopes.
- Do not design automotive-host around spawn-per-tool-call assumptions.
- Do not hard-wire automotive behavior into `rust/crates/tools`, `runtime`, or `rusty-claude-cli` unless the change is a generic extension primitive that would still make sense upstream.
- If a fork-side change is needed, keep it upstream-friendly, generic, documented, and isolated from automotive specifics.

## Source Of Truth Boundaries

- `automotive-host` owns:
  - session model
  - policy decisions
  - procedure execution
  - adapter behavior
  - audit records
  - host-local API contracts
- The `claw` fork owns:
  - orchestration
  - agent UX
  - generic tool runtime
  - generic plugin and MCP loading
- Never duplicate safety-critical rules across both systems.
- If a rule is automotive-specific, it belongs in `automotive-host`.

## Architecture Rules

- Session first: every operation must belong to a typed automotive session.
- Procedure first: risky workflows must be declared as procedures, not improvised in prompts.
- Policy before action: read, write, flash, and coding requests must be evaluated before execution.
- Audit by default: meaningful actions must emit durable audit evidence.
- Adapter isolation: each vendor software family stays behind its own adapter boundary.
- Same-machine zero trust: treat the local machine as hostile until identity, context, and preconditions are verified.
- Internal events and failures are owned by `automotive-host`; the bridge maps them to `claw` at the transport boundary.
- Public contracts must be typed JSON with stable versionable envelopes.
- Prefer dependency injection and explicit interfaces over hidden globals for host runtime code.

## Runtime Maturity Standards

- Process supervisor first: no real adapter runtime work should bypass owned process lifecycle, Job Object cleanup, or session/port locking.
- Replace preview-only routing with explicit handlers and typed error surfaces.
- Every external interaction must have timeouts, retries only when safe, and degraded-mode reporting.
- Failure modes must be structured and machine-readable, not buried in prose strings.
- Persist session and audit state behind explicit repositories, not ad hoc in-memory coupling.
- Add health, readiness, and capability reporting for the local host runtime.
- Make Windows assumptions explicit: process paths, user-session requirements, window identities, supported OS variants.
- UI Automation work must run on a dedicated automation worker thread owned by the session, not on the request-handling thread.
- Capture first, parse second: raw serial bytes are the source of truth, parser output is a derived layer.
- Keep dangerous operations idempotent where possible and always checkpointed where not.

## Integration Rules For The Fork

- Prefer restricting tool exposure through configuration and wrapper UX before deleting built-in `claw` tools.
- Use the fork for product packaging, branding, defaults, and allowed-tool policy.
- Do not add a local HTTP control surface in V1 unless a later ADR explicitly reopens that decision.
- Avoid modifying core built-in tool behavior for automotive-specific needs.
- If a core modification is unavoidable, it must satisfy all of:
  - useful beyond automotive
  - small and reviewable
  - covered by tests
  - unlikely to conflict with upstream updates

## Code Quality Expectations

- Every meaningful change must meet a senior Tier-1 engineering bar.
- Code should read like it was written for a critical enterprise system that will be reviewed by very strong peers and operated under stress.
- Default assumption: every change will receive a hostile review focused on edge cases, failure handling, maintainability, and hidden coupling.
- "Review-proof" means the code should be hard to reject on correctness, clarity, naming, invariants, and operational safety.
- Favor explicit domain names over generic helper abstractions.
- Keep serialization shapes deterministic and test them.
- Validate cross-field invariants at boundaries; do not rely on "well-formed caller" assumptions.
- Prefer enums and typed state machines over stringly-typed flags.
- Make illegal states hard to represent.
- Keep tests focused on contracts, safety rules, and failure classification, not only happy paths.
- Add regression tests whenever a bug reveals a missing invariant.

## Tier-1 Review Bar

- Design for long-term maintenance first, not for short-term convenience.
- Prefer boring, legible, strongly typed code over cleverness.
- Do not introduce abstraction unless it removes real duplication or isolates a real volatility boundary.
- Make failure handling explicit: no silent fallbacks, no swallowed errors, no vague `String` plumbing where a typed error should exist.
- Name things so a new senior engineer can understand intent without hunting across the codebase.
- Every boundary should answer: what are the inputs, invariants, outputs, failure modes, and audit consequences?
- Minimize ambient state. Prefer owned state, dependency injection, and narrow interfaces.
- Optimize for deterministic behavior before optimizing for throughput.
- If concurrency is introduced, document ownership, synchronization, cancellation, timeout behavior, and shutdown semantics.
- If Windows-specific behavior is assumed, encode the assumption in types, tests, docs, or explicit guards.

## Implementation Discipline

- Do not leave placeholder logic in production paths unless it is explicitly marked as stub behavior and covered by tests.
- Do not add TODO-shaped architecture into core paths without recording the intended invariant in docs or tests.
- Do not broaden public contracts casually; version and evolve them deliberately.
- Keep module responsibilities sharp. If a module starts doing two jobs, split it.
- If a decision affects safety, process ownership, transport guarantees, or audit integrity, prefer the stricter design.
- Before closing a task, ask: would a very critical reviewer say this is robust under crash, timeout, malformed input, duplicate requests, and partial failure?

## Delivery Order

1. Make `op-com` read-only execution real.
2. Introduce the local host runtime and persistence.
3. Promote procedure runner and approval lifecycle from model to runtime.
4. Add controlled write flows only after read-only maturity is proven.
5. Add the definitive `claw` bridge after host contracts are stable.

## Non-Negotiables

- No free-form shell as the main control plane for vendor actions.
- No raw GUI-coordinate automation as the main public abstraction.
- No irreversible action without policy, approval, checkpoint, and verification.
- No automotive business logic spread across the fork core.
- No "temporary" shortcuts that bypass typed contracts for dangerous flows.
