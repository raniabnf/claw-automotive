use serde::{Deserialize, Serialize};

use crate::sessions::{EcuIdentity, VehicleIdentity};
pub use crate::types::DtcRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterSupportLevel {
    Supported,
    Planned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterPhase {
    Launch,
    AttachInterface,
    Navigate,
    Extract,
    ExportReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterFailureKind {
    ExecutableMissing,
    LaunchFailed,
    WindowNotFound,
    WindowMismatch,
    InterfaceUnavailable,
    UnsupportedScreen,
    ExtractionFailed,
    OperatorBlocked,
    TimedOut,
    NotImplemented,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionKind {
    VehicleIdentity,
    EcuIdentity,
    DtcList,
    LiveData,
    ReportExport,
    ToolHealth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolHealth {
    Unknown,
    Ready,
    Degraded,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolLaunchSpec {
    pub executable_name: String,
    pub cli_args: Vec<String>,
    pub requires_user_session: bool,
    pub expected_process_names: Vec<String>,
    pub working_directory_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowIdentity {
    pub title_hint: String,
    pub class_name: Option<String>,
    pub automation_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationCheckpoint {
    pub checkpoint_id: String,
    pub summary: String,
    pub phase: AdapterPhase,
    pub expected_window: WindowIdentity,
    pub blocking_dialogs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionTarget {
    pub target_id: String,
    pub kind: ExtractionKind,
    pub source_hint: String,
    pub output_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveDataPoint {
    pub name: String,
    pub value: String,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportArtifact {
    pub artifact_kind: String,
    pub path_hint: String,
    pub format: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedAdapterOutput {
    pub vehicle: Option<VehicleIdentity>,
    pub ecu: Option<EcuIdentity>,
    pub dtcs: Vec<DtcRecord>,
    pub live_data: Vec<LiveDataPoint>,
    pub report: Option<ExportArtifact>,
    pub tool_health: ToolHealth,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterInvocationPlan {
    pub supported: bool,
    pub support_level: AdapterSupportLevel,
    pub typed_only: bool,
    pub requires_operator_presence: bool,
    pub phases: Vec<AdapterPhase>,
    pub outputs: Vec<ExtractionKind>,
    pub failure_modes: Vec<AdapterFailureKind>,
    pub notes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{AdapterFailureKind, AdapterPhase, AdapterSupportLevel, ExtractionKind};

    #[test]
    fn shared_adapter_contract_types_serialize_to_machine_values() {
        assert_eq!(
            serde_json::to_value(AdapterPhase::AttachInterface).expect("phase serializes"),
            json!("attach_interface")
        );
        assert_eq!(
            serde_json::to_value(AdapterFailureKind::WindowMismatch)
                .expect("failure kind serializes"),
            json!("window_mismatch")
        );
        assert_eq!(
            serde_json::to_value(AdapterSupportLevel::Planned).expect("support level serializes"),
            json!("planned")
        );
        assert_eq!(
            serde_json::to_value(ExtractionKind::ReportExport).expect("kind serializes"),
            json!("report_export")
        );
    }
}
