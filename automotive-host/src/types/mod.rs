//! Domain types owned by the host (not adapter scaffolding).
//!
//! See `DEVELOPING.md` for layer boundaries. Types here are **stable contracts**;
//! adapter-specific quirks stay under `crate::adapters`.

pub mod dtc;

pub use dtc::{
    normalize_dtc_vendor_text_v1, DtcCodeKind, DtcEcuAddress, DtcMappingConfidence, DtcRecord,
    DtcStandardRef, TAXONOMY_SCHEMA_VERSION,
};
