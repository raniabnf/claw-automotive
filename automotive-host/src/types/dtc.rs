//! Canonical diagnostic trouble code types and **normalization v1**.
//!
//! # Contract
//!
//! The wire shape and normalization rules are **normative** in `ROADMAP-v3.md`
//! (*Domain lexicon, diagnostic taxonomy, and reliability contracts*).
//! Changing behavior requires bumping [`TAXONOMY_SCHEMA_VERSION`] and updating
//! golden fixtures under `tests/fixtures/dtc_normalization_v1/`.
//!
//! # Layers
//!
//! - **Vendor surface** — lossless tool string when it differs from canonical.
//! - **Host canonical** — `code_canonical` + `code_kind` from this module.
//! - **Standards reference** — [`DtcStandardRef`] when the binding proves traceable.

use serde::{Deserialize, Serialize};

/// Bump this whenever normalization or mandatory fields change (see roadmap).
pub const TAXONOMY_SCHEMA_VERSION: u16 = 1;

/// Classification of how `code_canonical` was derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DtcCodeKind {
    ObdJ2012,
    UdsTriplet,
    VendorOpaque,
    Unknown,
}

/// Traceable mapping to a published code set when the binding supports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DtcStandardRef {
    Iso15031J2012,
    Iso14229Dtc,
    None,
}

/// Provenance of classification (GUI vs verified bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DtcMappingConfidence {
    ConfirmedByWire,
    ConfirmedByUi,
    Inferred,
    Unknown,
}

/// Optional ECU addressing as supplied by an adapter (no guessing).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DtcEcuAddress {
    pub logical: Option<String>,
    pub physical: Option<String>,
}

/// Normalized DTC row for `diag.read_dtcs` and adapter output (ROADMAP v1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DtcRecord {
    pub taxonomy_schema_version: u16,
    pub code_kind: DtcCodeKind,
    pub code_canonical: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_code_text: Option<String>,
    pub standard_ref: DtcStandardRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dtc_status_byte: Option<u8>,
    pub mapping_confidence: DtcMappingConfidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ecu_address: Option<DtcEcuAddress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_gloss_id: Option<String>,
    /// Legacy human-oriented line retained until all callers use `catalog_gloss_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_text: Option<String>,
}

impl DtcRecord {
    /// Classify a vendor-visible code string using **normalization v1** only (no wire bytes).
    #[must_use]
    pub fn from_vendor_text_v1(vendor_input: &str, confidence: DtcMappingConfidence) -> Self {
        let (kind, canonical, conf) = normalize_dtc_vendor_text_v1(vendor_input).unwrap_or((
            DtcCodeKind::VendorOpaque,
            preprocess_opaque_key(vendor_input),
            DtcMappingConfidence::Unknown,
        ));

        let final_conf = if conf == DtcMappingConfidence::Unknown {
            confidence
        } else {
            conf
        };

        let trimmed = vendor_input.trim();
        let nfc = unicode_nfc(trimmed);
        let vendor_code_text = if nfc == canonical.as_str() {
            None
        } else {
            Some(nfc)
        };

        Self {
            taxonomy_schema_version: TAXONOMY_SCHEMA_VERSION,
            code_kind: kind,
            code_canonical: canonical,
            vendor_code_text,
            standard_ref: if matches!(kind, DtcCodeKind::ObdJ2012) {
                DtcStandardRef::Iso15031J2012
            } else if matches!(kind, DtcCodeKind::UdsTriplet) {
                DtcStandardRef::Iso14229Dtc
            } else {
                DtcStandardRef::None
            },
            dtc_status_byte: None,
            mapping_confidence: final_conf,
            ecu_address: None,
            catalog_gloss_id: None,
            status_text: None,
            detail_text: None,
        }
    }

    /// Build a record from three **authenticated** UDS DTC bytes (big-endian triplet).
    #[must_use]
    pub fn from_uds_triplet_v1(
        b0: u8,
        b1: u8,
        b2: u8,
        vendor_code_text: Option<String>,
        status_byte: Option<u8>,
        confidence: DtcMappingConfidence,
    ) -> Self {
        let canonical = format!("0x{b0:02X}{b1:02X}{b2:02X}");
        Self {
            taxonomy_schema_version: TAXONOMY_SCHEMA_VERSION,
            code_kind: DtcCodeKind::UdsTriplet,
            code_canonical: canonical,
            vendor_code_text,
            standard_ref: DtcStandardRef::Iso14229Dtc,
            dtc_status_byte: status_byte,
            mapping_confidence: confidence,
            ecu_address: None,
            catalog_gloss_id: None,
            status_text: None,
            detail_text: None,
        }
    }
}

/// Pure normalization: same input → same `(kind, canonical, confidence_hint)`.
///
/// `confidence_hint` is `ConfirmedByUi` when J2012-shaped; callers promoting to
/// wire-confirmed should replace after comparing capture.
#[must_use]
pub fn normalize_dtc_vendor_text_v1(
    vendor_input: &str,
) -> Option<(DtcCodeKind, String, DtcMappingConfidence)> {
    let slug = preprocess_j2012_slug(vendor_input);
    if slug.is_empty() {
        return None;
    }

    if slug.len() == 5 && is_j2012_primary(&slug) {
        return Some((
            DtcCodeKind::ObdJ2012,
            slug.to_uppercase(),
            DtcMappingConfidence::ConfirmedByUi,
        ));
    }

    if slug.len() == 8 && is_j2012_extended(&slug) {
        return Some((
            DtcCodeKind::ObdJ2012,
            slug.to_uppercase(),
            DtcMappingConfidence::ConfirmedByUi,
        ));
    }

    Some((
        DtcCodeKind::VendorOpaque,
        preprocess_opaque_key(vendor_input),
        DtcMappingConfidence::Unknown,
    ))
}

fn preprocess_j2012_slug(vendor_input: &str) -> String {
    let t = unicode_nfc(vendor_input.trim());
    let upper = t.to_ascii_uppercase();
    let stripped = upper.strip_prefix("DTC").map_or(&upper[..], str::trim);
    stripped
        .chars()
        .filter(|c| !(c.is_whitespace() || *c == '-'))
        .collect()
}

fn preprocess_opaque_key(vendor_input: &str) -> String {
    unicode_nfc(vendor_input.trim())
}

fn unicode_nfc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfc().collect()
}

fn is_j2012_primary(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    if bytes.len() != 5 {
        return false;
    }
    if !matches!(bytes[0], b'P' | b'C' | b'B' | b'U') {
        return false;
    }
    if !matches!(bytes[1], b'0' | b'1' | b'2' | b'3') {
        return false;
    }
    bytes[2].is_ascii_hexdigit() && bytes[3].is_ascii_hexdigit() && bytes[4].is_ascii_hexdigit()
}

#[allow(clippy::redundant_closure_for_method_calls)] // clippy's fn-pointer suggestion breaks on Iterator<Item=&u8>
fn is_j2012_extended(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    if bytes.len() != 8 {
        return false;
    }
    if !matches!(bytes[0], b'P' | b'C' | b'B' | b'U') {
        return false;
    }
    bytes[1..8].iter().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct GoldenCase {
        input: String,
        expected_kind: String,
        expected_canonical: String,
        expected_confidence: String,
    }

    #[test]
    fn normalize_is_idempotent_for_obd() {
        let (_, c, _) = normalize_dtc_vendor_text_v1("P0420").expect("obd");
        let (_, c2, _) = normalize_dtc_vendor_text_v1(&c).expect("second");
        assert_eq!(c, c2);
    }

    #[test]
    fn golden_vectors_from_disk() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/dtc_normalization_v1");
        let entries = std::fs::read_dir(&dir).expect("fixtures dir");
        for entry in entries {
            let path = entry.expect("entry").path();
            if path.extension().is_some_and(|e| e == "json") {
                let raw = std::fs::read_to_string(&path).expect("read");
                let case: GoldenCase = serde_json::from_str(&raw).expect("json");
                let got = normalize_dtc_vendor_text_v1(&case.input).expect("norm");
                let exp_kind = match case.expected_kind.as_str() {
                    "obd_j2012" => DtcCodeKind::ObdJ2012,
                    "vendor_opaque" => DtcCodeKind::VendorOpaque,
                    other => panic!("unknown kind in {path:?}: {other}"),
                };
                let exp_conf = match case.expected_confidence.as_str() {
                    "confirmed_by_ui" => DtcMappingConfidence::ConfirmedByUi,
                    "unknown" => DtcMappingConfidence::Unknown,
                    other => panic!("unknown confidence in {path:?}: {other}"),
                };
                assert_eq!(
                    got.0, exp_kind,
                    "kind mismatch {:?} input {:?}",
                    path, case.input
                );
                assert_eq!(
                    got.1, case.expected_canonical,
                    "canonical {:?} input {:?}",
                    path, case.input
                );
                assert_eq!(
                    got.2, exp_conf,
                    "confidence {:?} input {:?}",
                    path, case.input
                );
            }
        }
    }

    #[test]
    fn dtc_record_serializes_mandatory_fields() {
        let r = DtcRecord::from_vendor_text_v1("P0420", DtcMappingConfidence::ConfirmedByUi);
        let v = serde_json::to_value(&r).expect("json");
        assert_eq!(v["taxonomy_schema_version"], 1);
        assert_eq!(v["code_kind"], "obd_j2012");
        assert_eq!(v["code_canonical"], "P0420");
        assert_eq!(v["standard_ref"], "iso15031_j2012");
    }
}
