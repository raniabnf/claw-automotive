# Developing `automotive-host`

This document orients contributors and future fork wiring. **Normative product
and architecture decisions** live in `ROADMAP-v3.md` and `AGENTS.md`. This file
explains **where code belongs** and **what must not drift** without an explicit
version bump.

## Crate layout (current vs target)

| Area | Path | Role |
|------|------|------|
| **Stable domain types** | `src/types/` | Contracts shared across adapters and bridge (e.g. `DtcRecord`). **Do not** encode Op-Com-only quirks here. |
| **Adapter bindings** | `src/adapters/` | Per-tool launch, navigation, extraction plans. All vendor-specific strings and selectors stay here or in future `opcom_selectors.rs`. |
| **Transport / preview** | `src/schemas.rs` | JSON envelopes and `OperationName`; **Horizon 0** will converge DTC reads into one `diag.read_dtcs` path (see roadmap drift section). |
| **Host orchestration** | `src/host.rs` | Today: `preview_dispatch`. Target: real `dispatch` + supervisor (roadmap). |
| **Bridge** | `src/bridge.rs` | Only mapping to `claw` shapes; no automotive business rules (ADR-010). |

## DTC taxonomy and normalization

- Implementation: `src/types/dtc.rs`.
- **Changing normalization or mandatory JSON fields** requires:
  1. Increment [`TAXONOMY_SCHEMA_VERSION`](src/types/dtc.rs) (or the constant in
     that module).
  2. Update the normative section in `ROADMAP-v3.md` if behavior changes.
  3. Add or adjust golden files under `tests/fixtures/dtc_normalization_v1/`.
  4. Ensure `cargo test` and `cargo clippy --all-targets -- -D warnings` pass.

Golden fixtures are **regression locks**: each `.json` is loaded by
`types::dtc::tests::golden_vectors_from_disk`.

## `DtcRecord` migration note

`NormalizedAdapterOutput` (`src/adapters/common.rs`) now uses the canonical
`DtcRecord` from `crate::types`. Legacy fields **`status_text`** and
**`detail_text`** remain optional for adapter-populated human hints until
playbooks/catalog ids subsume them (`ADR-011`).

## Commands

From this directory:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

(`automotive-host` is **not** a member of the `rust/` workspace; run these here
or add the crate to the workspace deliberately later.)

## Branching and `claw` integration

- **Fork CLI / `rust/crates/*`:** no automotive domain logic; only invoke the
  host broker when integration lands (roadmap H7).
- **Protocol changes:** bump wire version fields and document in `ROADMAP-v3`
  backlog; keep `bridge.rs` the single coupling surface to `claw` types.

## Related docs

- `ROADMAP-v3.md` — ADRs, horizons, acceptance criteria.
- `AGENTS.md` — quality bar for this repo.
