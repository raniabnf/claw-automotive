# Automotive Host Roadmap

Last updated: 2026-05-04
Review cycle: ROADMAP v3 — incorporates peer review feedback on v2.

## Purpose

`automotive-host` is the domain system for automotive diagnosis,
measurement, coding, and controlled write operations.

This document is the internal source of truth for:

- the product vision and non-negotiable constraints
- the architecture decisions and their rationale
- the development order and dependencies
- the acceptance criteria for each phase
- the numbered backlog with priority, status, and commit traceability
- the failure taxonomy and recovery contracts
- the integration contract with `claw-code`

This roadmap applies only to `automotive-host/`.
It does not authorize moving automotive business logic into the main
`claw-code` core.

## North Star

Build a Windows-first, same-machine automotive execution host that can:

- launch, supervise, and orchestrate vendor diagnostic software (Op-Com,
  Renolink, Lexia, Xentry) as a controlled, owned runtime
- expose typed, machine-readable operations to claw as the higher-level
  orchestrator
- intercept all diagnostic traffic in real time so claw can reason over
  live vehicle data
- keep dangerous actions behind explicit procedures, policy checks,
  approvals, and verification
- produce structured, normalized outputs regardless of which vendor
  software produced the raw data

The long-term target is not GUI clicking with a model.
The long-term target is:

- automotive-host is a **runtime broker** that owns the entire process
  tree, the serial channel, and the session state
- claw orchestrates through the broker via typed tool calls
- all diagnostic traffic is observable in real time
- one adapter per software family, one normalized output contract
- policy and procedure layers govern every action

## Non-Negotiable Constraints

### Platform

- Windows-first (Windows 7 through Windows 11 via the same public contracts)
- same machine as the automotive software
- user-session process, not Session 0 service automation
- no kernel-mode driver requirements beyond com0com for diagnostic traffic
  interception

### Safety

- no free-form shell execution for vendor actions
- no write, flash, or coding action without policy evaluation
- no irreversible action without a safe checkpoint and explicit approval
- no hidden operator involvement for dangerous actions
- every action that touches the vehicle produces audit evidence
- automotive-host owns the process tree — no unowned vendor processes

### Architecture

- keep automotive domain logic inside `automotive-host/`
- automotive-host defines its own internal event model and types —
  no structural dependency on claw-code internal types
- the bridge translates internal events to claw's schema at the transport
  boundary — this is the only coupling point
- use typed JSON contracts from day one
- isolate software-specific behavior inside adapters
- Op-Com (Fantomel edition) remains the diagnostic runtime — we orchestrate
  it, we do not replace it

### Transport

- primary control surface: persistent local broker protocol over stdio
- claw may launch automotive-host as a child process, but the automotive
  session channel is long-lived and must not be modeled as a one-shot
  "spawn-per-tool-call" helper
- observability: `.claw/automotive-state.json` file (polled, not pushed)
- future option: named pipes for lower-latency or independently supervised
  local IPC while preserving the same message envelopes
- no HTTP server in V1 — unnecessary attack surface for a process that
  controls vehicle operations

### Op-Com Integration Constraint

Op-Com (Fantomel Multi 200603a) is the diagnostic software. It runs on the
same Windows machine, under automotive-host's process supervision.
automotive-host controls it through two complementary layers:

1. **Windows UI Automation API** — structured control of the Op-Com GUI
   (launch, navigate, trigger actions, read results from controls)
2. **Diagnostic traffic interception** — man-in-the-middle on the transport path
   between Op-Com
   and the FTDI/PIC18F458 interface, capturing all diagnostic traffic in
   real time

Op-Com is never modified, patched, or reverse-engineered at the binary level.
We treat it as a black-box runtime with two observable surfaces: its GUI and
its serial traffic.

## Architecture Decisions

### ADR-000: Process Supervisor First

**Context.** Op-Com is a third-party Windows GUI process that opens a COM
port, communicates with vehicle hardware, and can crash, hang, spawn
unexpected dialogs, or leave zombie processes. If automotive-host does not
own the process lifecycle, it cannot guarantee session integrity, port
cleanup, or crash recovery.

**Decision.** A `ProcessSupervisor` module is the first runtime module built
(Horizon 0). It owns:

- **Process launch with Job Object.** Every vendor process is spawned inside
  a Windows Job Object (`win32job` crate, MIT). `limit_kill_on_job_close()`
  ensures that if automotive-host exits (crash, kill, or graceful shutdown),
  the entire Op-Com process tree is terminated. No orphans, no locked COM
  ports.

- **ProcessHandle.** Typed handle carrying: PID, child process tree, main
  window HWND, start time, current state, COM port association.

- **Single-instance lock.** Only one adapter process per COM port at any time.
  Enforced by a named mutex (`Global\automotive-host-COM3`). Prevents two
  sessions from fighting over the same interface.

- **Session lock.** Binds: process handle + COM port + vehicle VIN + audit
  trace ID. The lock is exclusive — no concurrent sessions on the same
  vehicle through the same adapter.

- **State machine.** Explicit lifecycle states:
  ```
  Idle → Launching → Attached → LinkSearching → LinkEstablished →
  Operating → Blocked → Failed → Shutdown
  ```
  Every state transition is logged and emitted as an internal event.

- **Heartbeat / watchdog.** Periodic check (configurable, default 2s):
  is the process alive? is the main window responsive (not hung)?
  is the COM port still open? Failure to respond within the deadline
  triggers recovery.

- **Emergency stop.** Kill the entire process tree via Job Object. Always
  available, even from a crashed state. Releases the COM port, closes
  the session lock, writes an audit record.

- **Crash recovery policy.** Per-adapter configurable:
  - `restart_once_then_escalate` (default)
  - `restart_n_times { max: 3, backoff_ms: 1000 }`
  - `escalate_immediately`
  - `wait_for_operator`

- **Binary allowlist.** The supervisor validates the path and optionally
  the SHA-256 hash of the launched binary before spawning. Unknown binaries
  are rejected with `adapter_launch` failure.

**Consequences.**

- Op-Com can never outlive automotive-host
- COM port deadlocks from crashed processes are eliminated
- concurrent sessions on the same port are structurally impossible
- crash recovery is automatic for known failure modes
- the process supervisor is the foundation for all adapter operations —
  nothing touches Op-Com without going through it

**Crate:** `win32job = "2"` (MIT, safe Windows Job Object API).
Gated behind `#[cfg(target_os = "windows")]`.

### ADR-001: Orchestration model — UI Automation + Serial Proxy

**Context.** We need claw to have total visibility into Op-Com's behavior:
what it sends to the vehicle, what it receives, and what it displays.

**Decision.** Two complementary integration layers:

- **Layer 1 — Windows UI Automation (`IUIAutomation` COM API).** Controls the
  Op-Com GUI structurally. The `uiautomation` Rust crate (v0.19+, Apache-2.0)
  wraps the Windows COM API. If a control lacks an Automation ID, fall back to
  Win32 API (`FindWindowExW`, `SendMessageW` for ListView/TreeView item
  enumeration). Vision/OCR is available as a last-resort diagnostic tool only,
  never as the primary control path.

  **Fallback strategy:**
  1. UI Automation (preferred — structured, resolution-independent)
  2. Win32 API (fallback — direct window messages, still structured)
  3. Vision/OCR (diagnostic only — screenshot + OCR to verify what UIA reports,
     never used to drive actions)

- **Layer 2 — Diagnostic traffic interception via com0com.** Virtual COM port
  pair, Op-Com
  connects to one side, automotive-host owns the relay between that virtual
  endpoint and the real FTDI VCP COM port exposed by the PIC18F458 interface.
  `hub4com` is acceptable for lab validation and bootstrap, but the production
  runtime target is an owned relay inside automotive-host. Every byte is copied
  bidirectionally AND logged to a raw binary capture file before any parsing
  occurs.

**Consequences.**

- Op-Com is never modified — it remains a black box
- claw gets both the user-facing results (via UI Automation) AND the raw
  protocol traffic (via diagnostic traffic interception) — full observability
- if UIA fails on a specific control, Win32 fallback is available without
  changing the adapter logic
- raw capture is always available even if the frame parser has bugs
- V1 keeps the official FTDI VCP/COM model that vendor software expects — no
  D2XX requirement and no binary patching
- the proxy path stays owned and observable — no silent direct connection from
  Op-Com to the real port outside automotive-host

**Alternatives rejected.**

- Direct PIC18F458 protocol reimplementation: replaces Op-Com.
- SendKeys/pixel-based automation: fragile, resolution-dependent.
- DLL injection/hooking: invasive, breaks with updates.
- Vision-first: unreliable for structured data extraction.

### ADR-002: Internal event model — decouple from claw types

**Context.** The initial ROADMAP v2 proposed reusing claw-code's `LaneEvent`
and `WorkerFailureKind` directly. Peer review correctly identified this as
premature structural coupling. claw-code is a fast-moving fork; if its event
schema changes, automotive-host should not break.

**Decision.** automotive-host defines its own internal event and failure types:

- `AutomotiveEvent` — internal event enum, independent of claw
- `AutomotiveFailureKind` — internal failure taxonomy
- `AutomotiveSessionState` — internal session state machine

The bridge module (`bridge.rs`) contains a one-way mapping function:

```rust
pub fn to_claw_lane_event(event: &AutomotiveEvent) -> serde_json::Value
pub fn to_claw_failure(failure: &AutomotiveFailureKind) -> serde_json::Value
```

This is the ONLY place where claw's schema appears. The mapping is a pure
function that takes internal types and returns JSON matching claw's expected
shape. If claw's schema changes, only `bridge.rs` is updated.

**Consequences.**

- automotive-host can be tested, packaged, and operated without claw
- claw schema changes affect only `bridge.rs`, not the core domain
- internal tests never import claw types
- the bridge mapping is independently testable

### ADR-003: Failure taxonomy

**Context.** Automotive failures need domain-specific classes with recovery
strategies.

**Decision.** `AutomotiveFailureKind` enum with 14 failure classes:

```
// Process supervision
supervisor_launch_failed      — binary not found, hash mismatch, or spawn error
supervisor_process_died       — process exited unexpectedly
supervisor_window_hung        — main window not responding to heartbeat
supervisor_concurrent_lock    — another session holds the port/vehicle lock

// Adapter
adapter_timeout               — operation did not complete within deadline
adapter_dialog_blocked        — unexpected dialog (error, warning, license)
adapter_navigation_failed     — UI element not found by UIA or Win32 fallback

// Vehicle
vehicle_no_connection         — OBD link not established
vehicle_comm_lost             — OBD link dropped during operation
vehicle_ecu_rejected          — ECU returned a negative response (NRC)
vehicle_protocol_error        — malformed frame on the traffic interception path

// Infrastructure
traffic_interception_failure  — com0com or traffic relay failure
policy_denied                 — action blocked by policy engine
audit_write_failure           — audit record could not be persisted
```

Each class maps to a `RecoveryStrategy`:
- `Retry { max_attempts, backoff_ms }`
- `RestartAdapter`
- `Escalate`
- `Abort`
- `WaitForOperator { timeout_s }`

### ADR-004: Transport — stdio first, no HTTP in V1

**Context.** automotive-host controls vehicle operations. A local HTTP server
on `127.0.0.1` is an unnecessary network surface for a process that performs
safety-critical actions. At the same time, a pure one-shot plugin tool model
is too weak for a runtime that needs heartbeat, state transitions, streaming,
and cancellation.

**Decision.**

- **Session bootstrap:** claw may launch automotive-host as a child process.
  That launch path is a bootstrap concern, not the definition of the runtime
  protocol.

- **Primary runtime channel:** persistent stdio JSON-lines while the broker
  process is attached to claw. Requests, responses, events, heartbeats, stream
  chunks, and cancellation all share the same line-delimited envelope format.

- **Transport fallback with the same protocol:** if claw's plugin lifecycle
  proves too one-shot to own a long-lived automotive session reliably, named
  pipes become the primary session channel without changing request, response,
  event, or state-file schemas.

- **Observability:** `.claw/automotive-state.json` written atomically on
  every state transition. Polled by clawhip or any external observer. No push
  mechanism needed.

- **Future option:** Windows named pipes (`\\.\pipe\automotive-host`) for
  lower-latency IPC if the file polling model proves insufficient. Named pipes
  are local-only by default on Windows and do not open a network surface.

- **HTTP deferred:** a local HTTP server may be added later if a web-based
  dashboard is needed, but it is never the primary control surface.

**Consequences.**

- zero network surface in V1
- transport supports long-lived ownership, heartbeat, and subscriptions
- no architectural dependency on one-shot plugin execution semantics
- the protocol is trivially testable (pipe JSON lines, read JSON lines)

### ADR-005: Diagnostic traffic interception — raw capture before parsing

**Context.** The Op-Com serial protocol over FTDI is not fully documented.
The initial ROADMAP v2 defined the frame parser as part of the same step as
the interception relay. If the parser is wrong, captured data could be lost or
misinterpreted.

**Decision.** Diagnostic traffic interception has three distinct concerns,
built in order:

0. **Transport topology (fixed before parsing work).**
   - the hardware-facing side remains the official FTDI VCP driver and the real
     COM port exposed by Windows 7/11
   - the vendor-app-facing side is a `com0com` virtual pair; Op-Com connects to
     the virtual port, never directly to the real FTDI port
   - automotive-host owns the relay between virtual and real ports; `hub4com`
     is acceptable for lab bring-up and parity checks, but not as the final
     runtime authority
   - there is no silent bypass: if the proxy path cannot be established,
     automotive-host reports a typed blocked/degraded state instead of allowing
     uncaptured traffic

1. **Relay discipline (built with the topology).**
   - relay handles are opened with overlapped I/O on Windows
   - the Win32 COMM API is the control surface for the hot path: `SetCommMask`,
     `WaitCommEvent`, `SetCommTimeouts`, and explicit error/status inspection
   - line-status and signal changes that matter to diagnosis or health
     (`EV_ERR`, `EV_BREAK`, `EV_CTS`, `EV_DSR`, `EV_TXEMPTY`) are recorded or
     propagated deliberately, not ignored as transport noise
   - FTDI tuning is versioned as a compatibility profile, not left as ad hoc
     machine folklore; this includes driver version, latency timer, and any
     required flow-control assumptions

2. **Raw capture (built first).** Every byte in both directions is timestamped
   and appended to a binary capture file (`.claw/captures/<session_id>.bin`).
   Format: `[u64 timestamp_us][u8 direction][u16 length][u8... data]`.
   This layer has zero protocol knowledge — it is a faithful recording.

3. **Frame parser (built second).** Reads from the same byte stream and
   attempts to identify frames, checksums, CAN arbitration IDs, and payloads.
   Parser errors do not affect the raw capture. The raw capture is the source
   of truth; the parser is a convenience layer.

**Consequences.**

- raw data is always preserved regardless of parser quality
- the parser can be developed iteratively using captured data as test fixtures
- protocol reverse-engineering can happen offline against capture files
- forensic replay of a diagnostic session is possible from the capture alone
- the V1 runtime stays compatible with third-party software that expects a
  standard COM port, because FTDI remains in VCP mode
- relay failure is a typed runtime condition, not a hidden fallback to
  uncaptured direct hardware access
- Windows 7 timing sensitivity becomes a documented compatibility concern
  rather than an implicit workstation tweak

### ADR-006: Adapter control strategy — UIA primary, Win32 fallback, vision diagnostic-only

**Context.** Op-Com is a .NET Framework WinForms application from the 2010s.
Some controls may have proper UI Automation support, others may not. A single
strategy is fragile, especially on Windows 7-era WinForms surfaces where
automation metadata quality is inconsistent.

**Decision.** Three-tier strategy, per control:

1. **UI Automation (primary).** Use stable semantic selectors first:
   `AutomationId`, control type, stable name patterns, and parent-chain context.
   Preferred because it is resolution-independent, DPI-independent, and works
   through RDP. UIA cache requests are allowed as a performance optimization,
   but cached data is never the sole authority for a state-changing action.

2. **Win32 API (fallback).** If a control has no automation ID and UIA cannot
   reliably identify it, fall back to `FindWindowExW` + `SendMessageW`
   (`LVM_GETITEMCOUNT`, `LVM_GETITEMTEXT`, `TVM_SELECTITEM`, etc.). Still
   structured, still reliable, but requires knowing the control class. This
   fallback is expected for classic WinForms `ListView`, `TreeView`, and status
   controls on Windows 7.

3. **Vision/OCR (diagnostic only).** Take a screenshot of a specific window
   region and run OCR (Tesseract or Windows OCR API). Used ONLY to verify
   what UIA/Win32 reports, never to drive actions. This is a debug tool.

4. **Selector packs are versioned.** `opcom_selectors.rs` records selector
   strategy per control and per runtime variant (`Win7`, `Win11`) so the access
   path is explicit and testable.

Each Op-Com control in `opcom_selectors.rs` declares its strategy:

```rust
pub struct ControlSelector {
    pub uia_automation_id: Option<String>,
    pub uia_name_pattern: Option<String>,
    pub uia_control_type: Option<ControlType>,
    pub win32_class: Option<String>,
    pub win32_parent_chain: Vec<String>,
    pub strategy: ControlStrategy,  // UiaFirst | Win32First | UiaOnly | Win32Only
}
```

**Consequences.**

- each control has a documented, testable access path
- UIA failures don't block the adapter — Win32 is always available for
  classic WinForms controls
- no action ever depends on pixel positions or screen resolution
- vision is available for debugging but cannot silently become the main path
- the selector strategy is explicitly designed for Win7-era WinForms behavior,
  not for idealized modern desktop metadata quality

### ADR-007: UI Automation threading discipline

**Context.** Microsoft UI Automation calls can become slow or hang when the
client mixes automation work with its own UI thread or spreads subscriptions
across inconsistent threads. Reliability depends on explicit threading
discipline, not just correct selectors. Microsoft guidance for UI Automation
clients specifically recommends a non-UI MTA thread and same-thread event
handler management.

**Decision.**

- each adapter session owns a dedicated automation worker thread
- that worker owns no windows and is never the app UI thread
- COM initialization for UI Automation happens explicitly inside that worker
  with `CoInitializeEx(..., COINIT_MULTITHREADED)`
- UIA element discovery, control reads, action invocation, and event
  subscription/removal all run on the automation worker, never on the request
  handling thread
- add/remove event handler calls happen on the same owned worker thread; the
  client does not spread handler registration/removal across multiple threads
- the supervisor and adapter communicate with the automation worker through
  typed channels/messages, not shared ad hoc UIA handles
- Win32 fallback calls follow the same worker-thread ownership model so one
  thread owns the window interaction strategy for a session
- marshaled UIA elements are treated as apartment-affine handles; if the worker
  restarts, apartment lifetime ends, or cross-bitness marshaling fails, the
  adapter re-discovers elements from HWND + selector instead of reusing stale
  handles

**Consequences.**

- UIA threading bugs are designed out early instead of discovered in dogfood
- add/remove event handlers occur on the same owned thread
- request handlers remain responsive even when UIA is slow
- timeout, heartbeat, and cancellation semantics stay under supervisor control
- cached or marshaled UIA handles are intentionally disposable; selector-driven
  rediscovery is the recovery path

### ADR-008: Interception strategy selected from detected transport mode

**Context.** `com0com` + owned relay is a strong production strategy when the
vendor software talks to the interface through a normal FTDI VCP COM port.
However, the discovery phase must not assume every vendor tool uses the same
transport surface. Some software may rely on `D2XX`, on hardware identity, or
on environment checks that make VM-assisted capture or virtual-port
interposition unreliable.

**Decision.**

- **Discovery must identify the active transport mode first.** `H1.1` records
  whether the software uses FTDI as `VCP`, `D2XX`, or another transport shape,
  and what interception options that enables.

- **`VCP` detected -> production-approved path.** Use diagnostic traffic
  interception through `com0com` plus an owned automotive-host relay. This is
  the default V1 production path for Op-Com.

- **`D2XX` detected -> investigation required, no automatic production choice.**
  The roadmap does not assume that `DLL proxy`, API shimming, or any equivalent
  interception method is automatically acceptable. The discovery output must
  describe loaded libraries, hardware-binding behavior, and whether a
  non-destructive same-machine interception strategy exists that still respects
  the black-box runtime rule.

- **VM + host-side capture is discovery-only.** During early reverse
  engineering, a developer may use a Windows VM with USB passthrough and a
  Linux host capture tool such as `socat` to observe tolerated traffic patterns.
  This is acceptable only as a lab aid for software that tolerates it in
  observed conditions, such as Op-Com. It is never the production architecture.

- **Production remains native Windows.** The runtime stays same-machine,
  native Windows, under automotive-host supervision. Virtualization is an
  optional discovery convenience, not a deployment component.

**Consequences.**

- the roadmap stops assuming that all vendors share the same transport surface
- Op-Com can still move quickly on the `VCP` path without overcommitting the
  design for `D2XX`-backed tools
- the production architecture remains compatible with software that rejects VMs
  or depends on hardware identity
- intrusive interception methods are not silently normalized into V1

### ADR-009: Canonical diagnostic operations above adapter bindings

**Context.** The host must expose stable automotive capabilities such as
reading DTCs without baking Op-Com-specific semantics into the public contract.
At the same time, not every read requires a full multi-step procedure.

**Decision.** The domain model has three explicit layers above the runtime:

- **Diagnostic operation.** Canonical, vendor-neutral verbs such as
  `diag.read_dtcs`, `diag.read_live_data`, `vehicle.discover`, and
  `ecu.identify`. These are the transport-facing contracts used by claw and by
  host-local orchestration.

- **Procedure definition.** Governed, multi-step workflows that compose
  operations with preconditions, approvals, checkpoints, and verification.
  Procedures are mandatory for risky or multi-step sequences, but they are not
  the only home for safe reads.

- **Adapter binding.** Software-specific implementation of a canonical
  operation for a given tool family and Windows version. The binding owns
  selectors, navigation, extraction, and software-specific failure modes.

`read_dtcs` is the canonical first slice:

- one request contract
- one normalized `DtcRecord` output shape
- one Op-Com binding in V1
- future Lexia/Renolink/Xentry bindings later without changing the public
  operation contract

**Consequences.**

- the bridge speaks stable business operations instead of vendor-specific verbs
- simple reads can execute directly inside a typed session after policy
  evaluation
- procedures remain reserved for governed sequences, not for every basic read
- adding a second diagnostic software family means writing another binding, not
  inventing a new public operation

### ADR-010: `claw-automotive` CLI mode — orchestration shell, not business runtime

**Context.** The product UX target is that an operator launches
`claw-automotive` from a terminal, like `codex`, `claude code`, or `claw`, then
asks for automotive work in natural language or through explicit scenarios.
The forked CLI must feel integrated enough to expose a coherent automotive
operator experience. At the same time, `claw-code` is still evolving its JSON
help, session, resume-safe, and agent-control surfaces. If automotive business
logic is placed inside the fork CLI, upstream churn can break the domain system
and make safety-critical rules hard to review.

**Decision.**

- **Product entrypoint:** `claw-automotive` may expose a dedicated CLI
  mode/subcommand in the fork so the operator experience is first-class.

- **Runtime ownership:** `automotive-host` remains the business runtime. It owns
  sessions, diagnostic operations, policies, procedures, audit, adapters,
  traffic interception, process supervision, and failure taxonomy.

- **Fork-side responsibility:** the fork CLI may only:
  - start or attach to an `automotive-host` broker
  - verify broker health and protocol version
  - route operator/agent requests to the host protocol
  - surface host state and errors without inventing automotive semantics
  - expose bridge tools or scenarios that are thin projections of host
    contracts

- **No fork-owned domain rules:** no VIN rules, DTC semantics, ECU policies,
  write approvals, adapter navigation, or traffic-capture interpretation may
  live in `rust/crates/tools`, `runtime`, or `rusty-claude-cli` unless the
  change is generic and useful outside automotive.

- **Fail closed:** if `automotive-host` is unavailable, unhealthy, or exposes an
  incompatible protocol version, the CLI reports a typed bridge failure. It
  does not silently fall back to shell commands, ad hoc prompts, or direct
  vendor-tool control.

- **Bridge isolation:** all coupling to `claw-code` lives behind a bridge client
  and schema mapper. If upstream `claw-code` changes its JSON/event surfaces,
  only that mapper is updated.

**Consequences.**

- the operator gets a native `claw-automotive` experience without sacrificing
  domain ownership
- `automotive-host` can be tested, reviewed, and operated without the fork CLI
- fork-side code stays small, generic, and reviewable
- the definitive fork bridge is intentionally blocked until host contracts are
  stable enough to avoid coupling to preview shapes
- the first implementation work can validate contracts, broker behavior,
  state, audit, and supervisor ownership with mock processes before any
  diagnostic software is automated

### ADR-011: Scenario catalog and DTC interpretation boundary

**Context.** Operators and agents need repeatable workflows (scenarios) that
survive tool and firmware churn. They also need explanatory guidance for DTCs.
If interpretation is left entirely to the LLM, answers drift, hallucinate, and
cannot be audited. If interpretation is hard-coded in the fork CLI, upstream
churn breaks safety-critical domain rules (contradicts ADR-010).

**Decision.**

- **Two-layer knowledge model.**
  - **Observation layer (facts):** `automotive-host` produces structured,
    versioned facts only: normalized `DtcRecord` (code, status, ECU id if known,
    timestamps, optional raw vendor strings for traceability), live data
    snapshots, traffic captures, and procedure/audit events. Facts are keyed by
    session and capture ids.
  - **Playbook layer (interpretation):** scenarios, decision rules, and DTC
    glosses live in a **catalog** owned by `automotive-host` (or an auditable
    data pack it loads). The catalog is data + schema, not prose in the model's
    weights. The LLM orchestrates; the catalog constrains and cites.

- **Scenario definition (minimum contract).** Each scenario has: stable `id`,
  semver `catalog_version`, applicability filters (optional: make, model, ECU
  family, adapter id), `preconditions`, ordered `steps` that reference
    canonical operations (`ADR-009`) or governed procedures, expected
    artifacts (e.g. DTC cleared, PID within range), and explicit `failure` /
    `abort` semantics. Scenarios are deterministic scripts over host operations,
    not free-form chat.

- **DTC « understanding » (first phase).** Phase 1 is **lexical + taxonomy**
    only: map codes to generic descriptions (SAE J2012 / ISO 15031-6 where
    applicable) and store manufacturer-specific extensions in the catalog with
    provenance. Phase 2 adds **playbook linkage**: which scenario ids apply,
    which measurements to collect next, and which procedures require policy.
    The bridge exposes compact JSON so `claw` tools never need to scrape the
    Op-Com UI for meaning—only for execution when the binding requires it.

- **Maintenance and improvement over time.** Catalog packs are **git-versioned**
    artifacts (or signed bundles with embedded version), validated by schema
    tests and golden fixtures (`session snapshot in → recommended next step
    out`). Changes ship with a changelog entry and catalog semver bump.
    Regression tests lock behavior; partial overrides (e.g. per-workshop
    additions) are explicit overlays with merge rules, not silent edits.

- **Vendor software note.** Public, stable automation APIs for Op-Com,
    Diagbox/Lexia, Renolink, Xentry are not assumed. Industry-consistent
    approaches are: same-machine orchestration, UI Automation, and diagnostic
    traffic capture (`ADR-001`, `ADR-005`, `ADR-008`). Any third-party **DTC
    text database** or ontology used for Phase 1 must respect its license;
    prefer permissively licensed or self-curated datasets for redistribution.

**Consequences.**

- `claw` / agents **reason over tool outputs + catalog version**, not over
  undocumented GUI strings as the source of truth
- safety and review focus on the catalog and host contracts, not on prompt
  wording
- scenario quality improves through versioned data and tests, not through model
  retraining alone
- alignment with ADR-009/010: operations stay canonical; interpretation stays
  host-owned or host-loaded

## Product Principles

1. **Own the process tree.**
   automotive-host is the process supervisor. Vendor software runs under
   its Job Object. No orphans, no zombies, no unowned processes.

2. **Orchestrate, don't replace.**
   Op-Com is the diagnostic runtime. We control it, observe it, and reason
   over its outputs. We do not reimplement its diagnostic logic.

3. **Two observation surfaces.**
   Every adapter operation is observable through both the GUI (UI Automation)
  AND the wire (diagnostic traffic interception). Discrepancies between the two
  are themselves diagnostic signals.

4. **Capture first, parse second.**
   Raw serial bytes are always preserved. The parser is a convenience on top.

5. **Session first.**
   Every operation belongs to a typed automotive session with a vehicle
   identity, a start time, and an audit trail.

6. **Operation first.**
   Common diagnostic capabilities are canonical domain contracts above
   software-specific adapter bindings.

7. **Procedure first.**
   Critical sequences are described as procedures, not improvised live.

8. **Policy before action.**
   The host decides allow, approval-required, or deny before execution.

9. **Read-first maturity.**
   Read-only capability must become reliable before write/coding capability.

10. **Same output shape across software.**
   Claw receives normalized structures regardless of which vendor software
   produced the raw data.

11. **Evidence over guesswork.**
    Every meaningful action produces audit data, evidence references, and
    verification artifacts.

12. **Failure is first-class.**
    Every failure mode has a typed class, a known recovery path, and an
    audit record. No opaque errors.

13. **Independence from claw internals.**
    Internal types are owned by automotive-host. The bridge maps to claw's
    schema at the transport boundary. Core domain survives claw fork changes.

## V1 Execution Scope

- real execution maturity starts with `Op-Com` only
- the first production target is Windows 7 + official FTDI VCP + PIC18F458
  firmware `1.67` + `com0com` traffic-interception topology
- generic operation contracts are defined now so future adapters can bind to
  them without changing the public surface
- `Lexia`, `Renolink`, and `Xentry` remain planned adapter bindings, not equal
  execution targets for the first shipped runtime slice
- the first complete business slice is: session + supervisor + Op-Com attach +
  canonical `diag.read_dtcs` + normalized `DtcRecord` output + audit evidence +
  active raw serial capture

## Canonical Domain Operation Model

The host is organized around explicit domain layers:

| Layer | Responsibility | Example |
|-------|----------------|---------|
| Diagnostic operation | Stable, vendor-neutral host contract | `diag.read_dtcs` |
| Procedure | Governed multi-step workflow | clear DTCs with checkpoint + verification |
| Adapter binding | Tool-specific implementation of an operation | Op-Com binding for `read_dtcs` |
| Normalized output | Tool-independent result contract | `Vec<DtcRecord>` |

Rules:

- simple reads may execute directly through the diagnostic-operation layer
  inside a typed session
- procedures compose operations; they do not replace them
- adapter bindings are the only place where vendor GUI and software quirks live
- the transport boundary must converge on one canonical DTC-read contract rather
  than multiplying overlapping request names
- the public wire contract is the `diag.*` family of operations; preview
  scaffold shapes such as `OperationName`, `ReadFaults`, or
  `DispatchAdapterOperation` are migration debt until Horizon 0 convergence
- invalid command combinations must be impossible or rejected at the boundary:
  a request parser/validator must not allow an action/payload mismatch to enter
  runtime execution

Boundary mapping:

| Layer | Scope | Stability target |
|-------|-------|------------------|
| `diag.*` transport op | public bridge / wire contract | canonical for V1 |
| diagnostic operation | internal domain capability | canonical for V1 |
| adapter operation | software-binding execution verb | adapter-local |
| procedure step | governed workflow step | procedure-local |

Interpretation rule:

- `diag.read_dtcs` is the public contract
- that contract resolves to one canonical internal read-DTC operation
- an adapter binding may implement that operation with `AdapterOperation::ReadDtcs`
- a procedure may invoke the same operation as one step among several, but does
  not redefine the public meaning of `diag.read_dtcs`

## Module Responsibilities

```
src/
├── main.rs                     — binary entrypoint, stdio JSON-line protocol
├── supervisor.rs               — ProcessSupervisor, Job Object, heartbeat, locks
├── host.rs                     — runtime-owned request routing and operation dispatch
├── schemas.rs                  — transport parser/validator and staged envelope types
├── operations.rs               — canonical diagnostic operations, normalized contract names
├── events.rs                   — AutomotiveEvent enum (internal event model)
├── failure.rs                  — AutomotiveFailureKind taxonomy, recovery mapping
├── sessions.rs                 — automotive session state, checkpoints, approval tickets
├── policy.rs                   — zero-trust evaluation rules
├── procedures.rs               — deterministic procedure definitions, preconditions
├── audit.rs                    — append-only audit model, evidence references
├── bridge.rs                   — claw integration: event mapping, state file writer
├── traffic_interception/
│   ├── mod.rs                  — interception lifecycle (setup, start, stop)
│   ├── relay.rs                — owned bidirectional byte relay (com0com ↔ real FTDI VCP port)
│   ├── capture.rs              — raw binary capture writer (ADR-005 layer 1)
│   └── parser.rs               — frame parser (ADR-005 layer 2)
├── ui_automation/
│   ├── mod.rs                  — shared helpers (find window, wait for element)
│   ├── process.rs              — process launch via supervisor, window attachment
│   ├── controls.rs             — typed wrappers: ListView, TreeView, Button, StatusBar
│   └── win32_fallback.rs       — Win32 API fallback for controls without UIA support
├── adapters/
│   ├── common.rs               — adapter binding metadata, `VehicleToolAdapter`, shared types
│   ├── opcom.rs                — Op-Com Fantomel adapter
│   ├── opcom_selectors.rs      — Op-Com window titles, control selectors (UIA + Win32)
│   ├── opcom_protocol.rs       — Op-Com serial frame parser
│   ├── renolink.rs             — placeholder
│   ├── lexia.rs                — placeholder
│   └── xentry.rs               — placeholder
└── types/
    ├── dtc.rs                  — DtcRecord, DtcStatus, DtcSeverity
    ├── ecu.rs                  — EcuDescriptor, EcuIdentity
    ├── vehicle.rs              — VehicleIdentity, VinRecord
    ├── live_data.rs            — LiveDataFrame, PidRequest, PidValue
    └── obd.rs                  — ObdLinkStatus, ObdProtocol
```

## Current Scaffold Drift To Resolve In Horizon 0

- the current scaffold already points in the right direction: generic
  operations exist, procedure steps can reference adapter operations, and
  normalized outputs already exist for DTCs
- `src/schemas.rs` still defaults `LocalTransport::LoopbackHttp`; this is
  preview-era debt, not a V1 architectural decision. V1 remains stdio-first
  with named pipes as the only planned local fallback
- older scaffolding and draft notes may still say `serial_proxy`; the canonical
  domain term is now `diagnostic traffic interception`
- the code currently uses `VehicleToolAdapter`, while earlier roadmap language
  said `DiagnosticAdapter`; converge on one naming set instead of multiplying
  concepts
- `ReadDtcs`, `ReadFaults`, and `DispatchAdapterOperation` overlap conceptually
  today; Horizon 0 must collapse this into one canonical DTC-read contract at
  the transport boundary
- the current action/payload split allows ambiguous combinations; Horizon 0
  must replace this with a discriminated union or an equally strict parse-time
  validator that rejects mismatched shapes before execution
- the current top-level `bridge.rs` scaffold only describes wrapper envelopes
  and descriptors; it is not yet the normative V1 bridge contract. Horizon 0
  must make the `diag.*` protocol, event mapping, and state-file writer the
  single source of truth
- the current `src/main.rs` binary is print-only (`--print-bridge`,
  `--print-adapters`, or host snapshot); it is not yet a long-lived broker and
  must not be treated as an integration-ready runtime
- the current `src/host.rs` path is `preview_dispatch()` and returns routing
  summaries or adapter plans; it is not an execution boundary and must not be
  used by fork-side CLI code as if it were production dispatch
- `BridgeDescriptor.stateless` applies only to the outer translation wrapper;
  the host process and automotive session channel remain stateful and
  long-lived. This distinction must be explicit in code and docs
- adapter-local planning failures and public `AutomotiveFailureKind` are not the
  same layer; Horizon 0 must document and implement the mapping boundary
- future fork-side `claw-automotive` CLI integration must wait for H0 contract
  artifacts instead of binding to preview structs, preview routes, or
  descriptor-only bridge metadata
- no fork-side CLI implementation may compensate for host scaffold gaps by
  duplicating domain logic in `rust/crates/tools`, `runtime`, or
  `rusty-claude-cli`
- this roadmap's target module map is ahead of the scaffold in some places;
  Horizon 0 is explicitly the convergence step that brings document and code
  back into alignment

## Roadmap Horizons

### Horizon 0 — Canonical Contracts + Broker Skeleton + Supervisor Mock

Status: next
Priority: P0

Goal:
Freeze the host contracts and turn the current typed scaffold into a minimal
runtime broker before any deep adapter execution work. Horizon 0 proves that
`automotive-host` can parse requests, reject invalid shapes, emit typed events
and failures, update state, write audit evidence, and own a supervised test
process without touching Op-Com or any real vehicle software.

Contract bar for Horizon 0:

- exactly one normative public wire contract for each business operation
- exactly one normative failure taxonomy at the transport boundary
- invalid transport shapes rejected before runtime dispatch
- stateless bridge-wrapper concerns clearly separated from the stateful host
  session channel
- broker behavior is testable with deterministic mock operations and mock
  supervised processes
- no real diagnostic software is required to complete Horizon 0

Current repo reality:

- `automotive-host` already contains a typed scaffold (`schemas`, `policy`,
  `procedures`, `bridge`, adapter descriptors)
- the scaffold already contains the seeds of the canonical operation model
  (`ReadDtcs`, normalized DTC output, procedure step binding)
- the current `host.rs` path is still preview-oriented rather than runtime-owned
- preview-era drift is still present and must be corrected in H0:
  - HTTP default in `schemas.rs`
  - overlapping request names around DTC reads
  - naming drift between roadmap terms and current adapter trait names
- Horizon 0 is the convergence step that turns the current scaffold into a real
  broker skeleton without discarding the typed contracts that already exist

Delivery structure:

- `H0.A` and `H0.B` are delivery bundles that define the order of work.
- `H0.2`, `H0.3`, `H0.4`, and `H0.C` remain the detailed work packages inside
  those bundles.
- If the same module appears in both places, the bundle states why it is needed
  now; the detailed work package states the exact acceptance criteria.

#### H0.A — Canonical contract layer

Implement the platform-neutral contracts first:

- `src/operations.rs`
  - `DiagnosticOperation` enum with canonical names such as `DiagReadDtcs`
  - stable wire names such as `diag.read_dtcs`
  - operation metadata: action class, read/write risk, output contract
  - parser from public op string to internal operation

- `src/events.rs`
  - `AutomotiveEvent` enum independent from claw internals
  - event metadata: `event_id`, `timestamp`, `session_id`, `trace_id`,
    `operation`, `sequence`
  - JSON round-trip and ordering tests

- `src/failure.rs`
  - `AutomotiveFailureKind`
  - `AutomotiveFailure`
  - `RecoveryStrategy`
  - mapping boundary from adapter-local failure to public failure

- `src/state.rs` or bridge-owned `AutomotiveState`
  - state snapshot serialized to `.claw/automotive-state.json`
  - status, current operation, last failure, session id, health, capture state

- request parser/validator
  - accepts JSON-lines protocol envelopes
  - rejects malformed JSON, unknown message type, unknown operation, missing
    correlation id, and invalid parameter shape
  - guarantees that invalid action/payload combinations cannot enter runtime

Acceptance:
- every public operation has one canonical string and one internal variant
- `diag.read_dtcs` parses to exactly one internal operation
- every request/response/event/failure/state shape round-trips through JSON
- malformed input returns typed JSON error and does not panic
- adapter-local failure kinds cannot serialize as public transport failures
- test: `diag_read_dtcs_contract_is_canonical`
- test: `malformed_protocol_line_returns_typed_error`
- test: `unknown_operation_rejected_before_dispatch`
- test: `events_failures_and_state_round_trip_json`

#### H0.B — Runtime broker skeleton + mock supervisor

Implement a deterministic broker/runtime skeleton without Op-Com:

- `src/main.rs`
  - replace print-only behavior with a persistent stdio JSON-lines loop
  - keep `--print-bridge` / `--print-adapters` as debug commands if useful
  - never write non-protocol logs to stdout while in broker mode

- `src/host.rs`
  - replace `preview_dispatch()` as the primary execution path with
    `dispatch()` backed by request validation, event emission, state updates,
    and audit writes
  - move preview behavior behind explicit stub handlers or tests

- `src/audit.rs`
  - add append-only audit writer abstraction
  - persist request accepted, operation started, operation completed/failed,
    state transition, and broker shutdown records

- `src/sessions.rs`
  - create/get/close session paths required for broker operation
  - session id and trace id become mandatory on operations that mutate host
    state

- `src/supervisor.rs`
  - define supervisor trait and state machine
  - implement a mock/test-process supervisor first
  - prove process ownership with a harmless child process, not a vendor
    diagnostic tool
  - Windows Job Object implementation can be behind `#[cfg(target_os =
    "windows")]`, with non-Windows tests exercising the trait and state machine

- state writer
  - atomic write via temporary file + rename
  - state file is valid JSON after every transition
  - write failures become typed `audit_write_failure` or state-write failure
    classes, never swallowed silently

Acceptance:
- broker reads multiple JSON-lines requests and writes correlated responses
- `cancel` for an in-flight mock operation produces a typed cancellation event
- state file updates after session start, operation start, failure, completion,
  and shutdown
- audit record exists for every meaningful broker action
- mock supervisor proves spawn/heartbeat/kill lifecycle without Op-Com
- test: `broker_handles_interleaved_event_and_response`
- test: `state_file_is_valid_json_after_every_transition`
- test: `audit_records_every_mock_operation`
- test: `mock_supervisor_kills_child_on_drop`

#### H0.C — Process supervisor core

Implement `src/supervisor.rs`:

Horizon 0 proves supervisor ownership with harmless test processes. Real Op-Com
launch/attach remains Horizon 1.3 after the broker and contracts are stable.

- `ProcessSupervisor::spawn(config: &AdapterLaunchConfig) -> Result<ProcessHandle>`
  - validates binary path exists
  - optionally validates binary SHA-256 against allowlist
  - creates a Windows Job Object with `limit_kill_on_job_close()`
  - spawns the process inside the Job Object
  - waits for the main window to appear (timeout: configurable, default 15s)
  - returns `ProcessHandle { pid, job, hwnd, com_port, start_time, state }`

- `ProcessHandle::heartbeat() -> Result<HeartbeatResult>`
  - checks: process alive (pid exists), window responsive (`SendMessageTimeout`
    with `SMTO_ABORTIFHUNG`), COM port still open
  - returns: `Alive`, `WindowHung`, `ProcessDead`, `PortLost`

- `ProcessHandle::kill() -> Result<()>`
  - terminates via Job Object (kills entire process tree)
  - releases COM port, named mutex, session lock
  - writes shutdown audit record

- `ProcessSupervisor::acquire_port_lock(port: &str) -> Result<PortLock>`
  - creates named mutex `Global\automotive-host-{port}`
  - returns error if another instance holds the lock

- State machine: `Idle → Launching → Attached → LinkSearching →
  LinkEstablished → Operating → Blocked → Failed → Shutdown`
  - every transition emits `AutomotiveEvent::SupervisorStateChanged`
  - every transition writes to `.claw/automotive-state.json`

- Convergence step against the current scaffold:
  - replace `preview_dispatch()` as the primary execution path with a real
    `dispatch()` path backed by supervisor state, typed failures, and event
    emission
  - preserve existing request/response contracts where still valid, and move
    preview-only behavior behind tests or explicit stub handlers

Dependencies: `win32job = "2"` (MIT), `windows` crate for `SendMessageTimeoutW`,
`CreateMutexW`, process enumeration.

Acceptance:
- if automotive-host is killed (`taskkill /F`), a supervised test child process
  dies within 1 second
- two simultaneous `spawn()` calls for the same COM port → second returns
  `supervisor_concurrent_lock`
- process that hangs (window not responding) → heartbeat returns
  `WindowHung` within 2 heartbeat cycles
- binary path that doesn't exist → `supervisor_launch_failed` with the
  path that was checked
- test: `job_object_kills_child_on_drop` (spawns notepad, drops handle,
  verifies notepad is dead)
- test: `port_lock_prevents_concurrent_spawn`
- test: `heartbeat_detects_hung_window` (spawn a test window, make it
  unresponsive, verify detection)
- test: `state_machine_transitions_emit_events`

#### H0.2 — Internal event model

Implement `src/events.rs`:

```rust
pub enum AutomotiveEvent {
    SupervisorStateChanged { from: SupervisorState, to: SupervisorState },
    AdapterLaunched { adapter: String, pid: u32 },
    AdapterCrashed { adapter: String, pid: u32, exit_code: Option<i32> },
    ObdLinkChanged { status: ObdLinkStatus },
    OperationStarted { operation: String, ecu: Option<String> },
    OperationCompleted { operation: String, ecu: Option<String>, summary: String },
    OperationFailed { operation: String, failure: AutomotiveFailureKind },
    SerialFrameCaptured { direction: Direction, length: usize },
    SessionStarted { session_id: String, vehicle: Option<VehicleIdentity> },
    SessionClosed { session_id: String, reason: CloseReason },
    PolicyDenied { operation: String, reason: String },
    ProcedureStepCompleted { procedure: String, step: u32, evidence_ref: String },
}
```

All events carry a `timestamp: DateTime<Utc>` and a `session_id: Option<String>`.

Acceptance:
- every event variant serializes to JSON and deserializes without loss
- events are independent of claw types — no claw imports in `events.rs`
- test: `all_event_variants_round_trip_json`

#### H0.3 — Bridge contract (claw mapping + state file)

Implement `src/bridge.rs`:

- `to_claw_event_json(event: &AutomotiveEvent) -> serde_json::Value`
  maps internal events to claw-compatible JSON shape
- `write_state_file(state: &AutomotiveState, path: &Path) -> Result<()>`
  atomically writes `.claw/automotive-state.json`
- canonical transport verbs are frozen here: `diag.read_dtcs` means the same
  business operation regardless of which adapter implements it; the bridge does
  not expose `opcom_*` transport verbs
- the top-level bridge remains a stateless translation layer only; long-lived
  session state lives in the host runtime, never in the wrapper descriptor
- preview scaffold envelopes such as `HostRequestEnvelope` are allowed only as
  migration helpers until the public `diag.*` parser/validator is in place;
  they are not the normative V1 wire contract

State file format:

```json
{
  "status": "idle | launching | attached | link_searching | link_established | operating | blocked | failed | shutdown",
  "adapter": "opcom",
  "adapter_pid": 12345,
  "vehicle": null,
  "obd_link": "not_connected",
  "current_operation": null,
  "last_failure": null,
  "traffic_interception": "inactive",
  "capture_bytes": 0,
  "session_id": null,
  "updated_at": "2026-04-11T14:30:00Z"
}
```

stdin/stdout protocol (JSON-lines):

```
→ {"type":"request","id":"req-1","op":"diag.read_dtcs","params":{"ecu":"engine"}}
→ {"type":"cancel","id":"req-2"}
← {"type":"response","id":"req-1","ok":true,"result":{"dtcs":[...]}}
← {"type":"response","id":"req-1","ok":false,"error":{"kind":"vehicle_no_connection","message":"..."}}
← {"type":"event","session_id":"session-1","event":{"kind":"operation_started","operation":"diag.read_dtcs"}}
← {"type":"stream","id":"req-2","seq":1,"chunk":{"pid":"0C","value":812,"unit":"rpm"}}
← {"type":"final","id":"req-2"}
← {"type":"heartbeat","session_id":"session-1","state":"operating","updated_at":"2026-04-11T14:30:00Z"}
```

Acceptance:
- bridge mapping function is a pure function with no side effects
- state file is valid JSON at every write (atomic rename)
- stdin/stdout protocol handles malformed input gracefully (returns error
  JSON, does not crash)
- invalid action/payload combinations are unrepresentable or rejected before
  dispatch
- protocol tolerates interleaved `event` and `response` messages without losing
  request correlation
- cancellation is explicit and typed, not implicit process kill
- test: `bridge_maps_all_event_variants`
- test: `state_file_is_valid_json_after_every_transition`
- test: `stdin_malformed_json_returns_error_not_crash`
- test: `invalid_request_shape_rejected_before_dispatch`
- test: `interleaved_events_do_not_break_response_correlation`
- test: `cancel_request_stops_stream_cleanly`

#### H0.4 — Failure taxonomy

Implement `src/failure.rs`:

- `AutomotiveFailureKind` enum with 14 classes (per ADR-003)
- `RecoveryStrategy` enum: `Retry`, `RestartAdapter`, `Escalate`, `Abort`,
  `WaitForOperator`
- `AutomotiveFailure` struct: kind + message + recovery + optional cause
- `Display` impl: one-line summary suitable for channel output
- `Serialize`/`Deserialize`: round-trips for bridge transport
- adapter-local planning failures (for example `NotImplemented`,
  `WindowNotFound`, `ExtractionFailed`) remain internal binding concerns until
  they are translated into one public `AutomotiveFailureKind` at the adapter
  boundary; they must never leak as competing top-level taxonomies

Acceptance:
- every failure class has a default recovery strategy
- test: `all_failure_kinds_display_and_serialize`
- test: `failure_with_nested_cause_preserves_chain`
- test: `adapter_failure_maps_to_single_public_failure_taxonomy`

#### H0.5 — CI pipeline

`.github/workflows/rust-ci.yml`:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (excluding `live_opcom` feature tests)
- `unsafe_code = "forbid"` enforced (already in Cargo.toml)

Acceptance:
- CI runs on every push to `main` and on every PR
- CI blocks merge on any failure

### Horizon 1 — Op-Com Discovery + UI Automation Foundation

Status: blocked by H0
Priority: P0

Goal:
Capture the Op-Com GUI map and build the UI Automation infrastructure with
Win32 fallback, explicitly tuned for Windows 7-era WinForms behavior.

#### H1.1 — Op-Com discovery pack (manual, committed as data)

Manual investigation. Capture and commit as `docs/opcom-gui-map.md`:

- installation path for Fantomel Op-Com Multi 200603a
- executable name, required firmware version (1.67)
- FTDI VCP driver version/provider, current latency timer, and real COM port
  naming on Windows 7
- detected FTDI transport mode for the software under test: `VCP`, `D2XX`, or
  other, plus the interception strategy that mode permits
- main window class name and title
- full control tree dump (via `uiautomation` tree walker)
- per-control: class, name, automation ID, control type, parent chain
- navigation path: launch → select vehicle → select ECU → read DTCs
- navigation path: launch → select ECU → measuring blocks (live data)
- per-control: document whether UIA or Win32 fallback is needed
- Win7 vs Win11 differences

Discovery note:

- for initial capture and reverse engineering, a developer may use a Windows VM
  (VMware/QEMU) with USB passthrough plus host-side Linux capture tooling such
  as `socat` to observe tolerated FTDI traffic from the developer workstation
  environment
- this lab workflow is considered valid only for software that tolerates it in
  observed conditions, such as Op-Com
- it is not assumed to generalize to all vendor software; some tools may bind
  to hardware identity or reject virtualization
- production architecture remains native Windows on the same machine as the
  diagnostic software; VM use is discovery-only

Acceptance:
- every control used in automation has a documented access strategy
- the document is reviewed against a live Op-Com session

#### H1.2 — UI Automation infrastructure

Implement `src/ui_automation/`:

- `mod.rs`: `find_window_by_title()`, `find_child()`, `wait_for_element()`
- `process.rs`: launch via `ProcessSupervisor`, wait for main window
- `controls.rs`: typed wrappers for ListView, TreeView, Button, StatusBar,
  Dialog detection
- `win32_fallback.rs`: `FindWindowExW` + `SendMessageW` for ListView
  item enumeration (`LVM_GETITEMCOUNT`, `LVM_GETITEMTEXT`), TreeView node
  selection (`TVM_SELECTITEM`)
- dedicated non-UI MTA automation worker thread per session, following ADR-007

Each control wrapper tries UIA first, falls back to Win32, logs which
strategy succeeded.

Acceptance:
- integration test against Notepad validates the infrastructure
- `wait_for_element()` returns `adapter_timeout` after deadline
- unexpected dialogs → `adapter_dialog_blocked` with dialog title and text
- test: `notepad_launch_find_window`
- test: `wait_for_element_timeout`

#### H1.3 — Op-Com adapter: launch + attach

Implement `opcom.rs` `discover()` + `launch()` using the supervisor:

- `discover()`: check install path, firmware version file, COM port
- `launch()`: `supervisor.spawn(opcom_config)`, wait for main window,
  detect initial connection state

Acceptance:
- launch creates Op-Com inside Job Object — killing automotive-host
  kills Op-Com
- missing install → `supervisor_launch_failed` with path checked
- unexpected startup dialog → `adapter_dialog_blocked`
- test: `opcom_discover_not_installed` (unit, no Op-Com needed)
- test: `opcom_launch_end_to_end` (tagged `live_opcom`)

#### H1.4 — OBD link detection

Implement `detect_obd_link()`:

- read status bar / connection indicator via UIA (with Win32 fallback)
- map to `ObdLinkStatus`: `NotConnected`, `Searching`, `Established`,
  `Error`

Acceptance:
- test: `obd_status_string_mapping` (unit, pattern matching)

#### H1.5 — Canonical `diag.read_dtcs` end-to-end (Op-Com binding)

First complete read-only business slice: canonical `diag.read_dtcs` enters the
host contract, resolves to the Op-Com adapter binding, navigates ECU tree,
triggers DTC read, extracts the ListView, and returns normalized
`Vec<DtcRecord>`.

Acceptance:
- empty DTC list → `Ok(vec![])`, not an error
- response shape is vendor-neutral `Vec<DtcRecord>` with no Op-Com-specific
  transport fields
- timeout → `adapter_timeout` with the step name
- result emitted as `AutomotiveEvent::OperationCompleted`
- the slice is only considered V1-complete when the same session also runs with
  active raw capture from H2.2 — no silent uncaptured happy path
- test: `dtc_record_serialization`
- test: `read_dtcs_end_to_end` (tagged `live_opcom`)

### Horizon 2 — Diagnostic Traffic Interception + Live Traffic Capture

Status: blocked by H1
Priority: staged (`H2.1-H2.2` = P0, `H2.3-H2.4` = P1)

Goal:
Full observability of all diagnostic traffic. The first deliverable is an owned
raw-capture proxy for the same read-only slice as H1.5; frame parsing and live
subscriptions come only after that transport is stable.

#### H2.1 — com0com setup automation

Implement `src/traffic_interception/mod.rs`:

- `detect_com0com()`: registry check, returns installed/version
- `detect_ftdi_vcp_profile()`: driver provider/version, latency timer,
  physical COM port, and any flow-control assumptions
- `create_pair()`: creates virtual COM pair, idempotent
- canonical pair naming for the Op-Com path (example: one vendor-facing virtual
  port and one host-facing virtual port), recorded in config and audit
- setup guide documentation
- approved installer/source recording: document the exact installer source,
  version, hash, signing state, and Win7/Win11 compatibility notes used by
  the project so the serial layer remains reproducible and auditable
- `hub4com` documented as an optional bootstrap/lab tool only; production relay
  acceptance belongs to automotive-host's own bridge implementation

Acceptance:
- test: `com0com_detection_structured_result`
- deliverable: `docs/com0com-setup.md` with compatibility matrix and approved
  installer metadata
- deliverable: compatibility matrix includes cable firmware `1.67`, FTDI VCP
  driver profile, virtual-pair naming, and approved latency setting

#### H2.2 — Raw capture layer (ADR-005 layer 1)

Implement `src/traffic_interception/capture.rs`:

- binary format: `[u64 timestamp_us][u8 direction][u16 length][u8... data]`
- one file per session: `.claw/captures/<session_id>.bin`
- append-only, flushed after every write
- no protocol knowledge at this layer

Implement `src/traffic_interception/relay.rs`:

- bidirectional relay: read from one port, write to the other, feed
  capture writer
- open both sides with overlapped I/O and drive the hot path through the Win32
  COMM API (`SetCommMask`, `WaitCommEvent`, `SetCommTimeouts`)
- explicitly handle disconnects, line-status errors, and signal changes rather
  than treating them as opaque transport failures
- no direct-connect fallback from Op-Com to the real FTDI port if the relay
  fails; the session transitions to a typed blocked/degraded state
- latency budget: < 1ms per relay

Acceptance:
- Op-Com works identically through the bridge
- every byte is captured with microsecond timestamp
- FTDI disconnect or interception failure → typed
  `traffic_interception_failure` plus state
  file update, not a silent stall
- direct-connect fallback is prohibited and covered by tests
- test: `bridge_relays_loopback` (com0com, no hardware)
- test: `capture_file_format_round_trip`
- performance: `bridge_latency_under_1ms`

#### H2.3 — Frame parser (ADR-005 layer 2)

Implement `src/traffic_interception/parser.rs`:

- parse raw stream into frames (delimiters, length, checksum)
- extract CAN arbitration ID and payload
- parser errors logged but do not affect raw capture
- developed iteratively against captured data from H2.2

Acceptance:
- test: `parse_captured_dtc_read_fixture`
- test: `partial_frame_reassembly`
- test: `checksum_error_produces_protocol_error_not_crash`

#### H2.4 — Live data subscription

Combine UI Automation (navigate to measuring blocks) + diagnostic traffic
interception
(capture live data stream) + subscription API.

Acceptance:
- claw can subscribe to PIDs, receive filtered frames
- `.claw/automotive-state.json` updated with latest values
- OBD link loss → `vehicle_comm_lost`

### Horizon 3 — Session Management + Audit Trail

Status: blocked by H0
Priority: P0

Delivery note:
H3.1 and H3.2 are part of the first governed read-only vertical slice. They do
not wait for parser/live-data maturity.

#### H3.1 — Vehicle session lifecycle

- create/get/close session, JSONL persistence
- session lock binds: process + port + VIN + audit trace

#### H3.2 — Audit trail

- append-only JSONL per session
- every operation, every failure, every frame count
- evidence references to capture files and DTC snapshots

### Horizon 4 — Procedure Engine + Policy Layer

Status: blocked by H3
Priority: P2

#### H4.1 — Procedure definitions (YAML + validation)
#### H4.2 — Policy engine (ReadOnly / WriteVehicle / FlashEcu / CodingChange)

### Horizon 5 — Controlled Write Operations

Status: blocked by H4
Priority: P2

#### H5.1 — Clear DTCs with before/after verification

### Horizon 6 — Multi-Software Adapters

Status: blocked by H5
Priority: P3

Renolink → Lexia → Xentry, same adapter pattern.

### Horizon 7 — Claw Integration Wiring

Status: blocked by H0 contracts + broker stability
Priority: P3

Goal:
Add the `claw-automotive` product CLI mode inside the fork without moving any
automotive business logic out of `automotive-host`.

#### H7.1 — Fork-side CLI entrypoint

Add a dedicated CLI mode/subcommand in `rusty-claude-cli`:

- exposes the product entrypoint (`claw-automotive` packaging or an automotive
  mode/subcommand in the fork)
- starts or attaches to `automotive-host`
- performs health, protocol-version, and capability checks
- fails closed if the broker is unavailable or incompatible
- emits structured JSON/text errors that follow the active `claw-code` error
  contract

Non-goals:

- no VIN, DTC, ECU, vehicle, procedure, adapter, or traffic-capture logic in the
  fork CLI
- no direct UI Automation, Win32, COM-port, or vendor-process control from the
  fork
- no shell fallback that bypasses `automotive-host`

#### H7.2 — Automotive bridge tool surface

Expose only thin bridge tools/scenarios:

- `automotive_host.dispatch` forwards validated host requests
- tool schemas mirror host contract versions
- every response carries host-owned `request_id`, `trace_id`, `session_id`,
  failure kind, and recovery metadata where applicable
- tool output is treated as a host projection, not as fork-owned truth

Acceptance:

- fork-side bridge tests prove requests are forwarded unchanged except for
  transport wrapping
- contract-version mismatch returns a typed error before any operation is sent
- broker unavailable returns a typed error with remediation, not prose-only
  guidance
- host state file remains the source of truth for automotive status
- test: `automotive_cli_does_not_construct_domain_decisions`
- test: `automotive_bridge_rejects_protocol_version_mismatch`
- test: `automotive_bridge_forwards_diag_read_dtcs_contract`

#### H7.3 — Operator UX

The CLI may provide automotive-specific ergonomics only as projections of host
truth:

- show active session and broker health
- list available host operations and procedures
- show current vehicle/session context if provided by the host
- render host failures and recovery hints
- provide explicit "host not ready" and "contract mismatch" states

The CLI must not infer vehicle state by scraping terminal output or guessing
from `claw-code` session state.

## Immediate Backlog

Priority: P0 = blocks first end-to-end, P1 = blocks live traffic,
P2 = blocks write ops, P3 = multi-software + claw wiring.

### P0 — Supervisor + Bridge + First Read Slice

| # | Item | Status | Commit | Date |
|---|------|--------|--------|------|
| 1 | Canonical contract layer (H0.A) | open | — | — |
| 2 | Runtime broker skeleton + mock supervisor (H0.B) | open | — | — |
| 3 | Process supervisor core with test process (H0.C) | open | — | — |
| 4 | Internal event model (H0.2) | open | — | — |
| 5 | Bridge contract + state file (H0.3) | open | — | — |
| 6 | Failure taxonomy (H0.4) | open | — | — |
| 7 | CI pipeline (H0.5) | open | — | — |
| 8 | Op-Com GUI map capture (H1.1) | open | — | — |
| 9 | UI Automation infra + Win32 fallback (H1.2) | open | — | — |
| 10 | Op-Com launch + attach via supervisor (H1.3) | open | — | — |
| 11 | OBD link detection (H1.4) | open | — | — |
| 12 | Canonical `diag.read_dtcs` end-to-end (H1.5) | open | — | — |
| 13 | com0com setup automation (H2.1) | open | — | — |
| 14 | Raw capture layer (H2.2) | open | — | — |
| 15 | Serial bridge relay (H2.2) | open | — | — |
| 16 | Session lifecycle (H3.1) | open | — | — |
| 17 | Audit trail (H3.2) | open | — | — |

### P1 — Parser + Live Data

| # | Item | Status | Commit | Date |
|---|------|--------|--------|------|
| 14 | Frame parser (H2.3) | open | — | — |
| 15 | Live data subscription (H2.4) | open | — | — |

### P2 — Policy + Write

| # | Item | Status | Commit | Date |
|---|------|--------|--------|------|
| 18 | Procedure definitions (H4.1) | open | — | — |
| 19 | Policy engine (H4.2) | open | — | — |
| 20 | Clear DTCs with verification (H5.1) | open | — | — |

### P3 — Multi-Software + Claw Wiring

| # | Item | Status | Commit | Date |
|---|------|--------|--------|------|
| 21 | Renolink adapter (H6) | open | — | — |
| 22 | Lexia adapter (H6) | open | — | — |
| 23 | Xentry adapter (H6) | open | — | — |
| 24 | Fork-side `claw-automotive` CLI entrypoint (H7.1) | open | — | — |
| 25 | Automotive bridge tool surface (H7.2) | open | — | — |
| 26 | Operator UX projections from host truth (H7.3) | open | — | — |

## Dependencies

### Rust Crates

| Crate | Version | License | Purpose |
|-------|---------|---------|---------|
| `serde` | 1.x | MIT/Apache-2.0 | Serialization |
| `serde_json` | 1.x | MIT/Apache-2.0 | JSON transport |
| `win32job` | 2.x | MIT | Windows Job Objects (process supervisor) |
| `uiautomation` | 0.19+ | Apache-2.0 | Windows UI Automation COM wrapper |
| `serialport` | 4.x | MIT | COM port enumeration and non-hot-path helpers |
| `windows` | 0.58+ | MIT/Apache-2.0 | Win32 API (fallback controls, mutexes, overlapped COMM hot path) |
| `chrono` | 0.4.x | MIT/Apache-2.0 | Timestamps |
| `tracing` | 0.1.x | MIT | Structured logging |

### External Software

| Software | Version | License | Purpose | Install scope |
|----------|---------|---------|---------|---------------|
| Op-Com Multi (Fantomel) | 200603a | — | Diagnostic runtime | User machine |
| FTDI CDM / VCP driver | approved matrix | — | Real hardware COM exposure | User machine |
| com0com | 3.0 (signed) | GPL | Virtual COM port pairs | One-time admin |
| hub4com | approved matrix | GPL | Lab/bootstrap relay validation | User machine |
| Op-Com V5 interface | FW 1.67 | — | Hardware | User hardware |

### Conditional Compilation

All Windows-specific code gated behind `#[cfg(target_os = "windows")]`.
Schemas, events, failure taxonomy, bridge contract compile and test on
all platforms. Only supervisor, UI Automation, traffic interception, and adapters
require Windows.

## Risk Register

| Risk | Impact | Prob | Mitigation |
|------|--------|------|------------|
| Op-Com controls lack automation IDs | H1 delayed | Med | Win32 fallback (ADR-006) covers all classic WinForms controls |
| com0com driver unsigned on future Win11 | H2 blocked | Low | Fallback: HHD Free Virtual Serial Ports (user-mode, signed) |
| Op-Com detects virtual COM port | H2 broken | Low | com0com ports are kernel-level, indistinguishable from real ports |
| Serial protocol undocumented framing | H2.3 delayed | Med | Raw capture (ADR-005) means parser bugs don't lose data |
| FTDI VCP timing or latency settings skew relay behavior on Win7 | H2/H1.5 unstable | Med | versioned FTDI compatibility matrix, approved latency profile, and live validation on the real cable |
| hub4com bootstrap behavior differs from the owned relay | H2 false confidence | Low | use hub4com for lab bring-up only; production acceptance runs on automotive-host relay |
| Fantomel releases new GUI version | H1 selectors break | Low | Selectors isolated in `opcom_selectors.rs`, version-specific sets |
| Job Object not supported on Win7 | H0.1 broken | Low | Job Objects exist since Windows XP; `win32job` supports Win7+ |
| claw-code changes event schema | bridge.rs update | Med | Only `bridge.rs` mapping functions need updating (ADR-002) |

## What We Will Not Do

- replace Op-Com with a direct PIC18F458 driver
- reverse-engineer Op-Com binaries or inject DLLs
- use vision/OCR as a primary control path
- expose a local HTTP server in V1
- require D2XX or replace the COM-port/VCP model in V1
- import claw-code internal types as source-of-truth dependencies
- build autonomous write/flash/coding before read-only maturity
- add adapters for other software before Op-Com is fully operational

## Development Rules

- every backlog item gets a commit hash and date when completed
- every dogfood incident gets a root-cause entry with: root cause, false
  leads, actual fix, meta-lesson (same format as claw-code ROADMAP)
- every new adapter starts with typed contracts and failure modes
- every software-specific rule stays inside its adapter module
- every meaningful milestone ends with green CI
- if the roadmap changes, update this file first

## Success Definition

automotive-host is on track when:

- claw can issue canonical `diag.read_dtcs`, automotive-host can launch Op-Com
  (via supervisor), verify OBD connection, and return normalized results — all
  without human intervention
- if automotive-host crashes, Op-Com dies cleanly (Job Object guarantee)
- diagnostic traffic interception captures all traffic in raw binary, and the
  parser can
  decode known frame types on top of it
- every operation produces an audit trail with evidence references
- adding a new software family means writing a new adapter, not rewriting
  the host or the supervisor
- claw consumes the system through a stdio bridge that only translates
  and forwards typed requests
- automotive-host can be tested and operated independently of claw
