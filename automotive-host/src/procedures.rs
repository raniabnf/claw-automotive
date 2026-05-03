use serde::{Deserialize, Serialize};

use crate::adapters::{AdapterOperation, ToolFamily};
use crate::sessions::{ActionClass, ApprovalKind, AutomotiveSession, BatteryState, IgnitionState};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcedurePrecondition {
    ToolAttached,
    VehicleIdentified,
    EcuIdentified,
    IgnitionOn,
    StableBattery,
    SafeCheckpointCaptured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcedureStepKind {
    SafeRead,
    RequestApproval,
    ExecuteWrite,
    VerifyPostAction,
    FailureExit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcedureStep {
    pub step_id: String,
    pub kind: ProcedureStepKind,
    pub summary: String,
    pub tool_family: Option<ToolFamily>,
    pub adapter_operation: Option<AdapterOperation>,
    pub action_class: Option<ActionClass>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcedureDefinition {
    pub procedure_id: String,
    pub display_name: String,
    pub required_tool: ToolFamily,
    pub required_ecu_family: Option<String>,
    pub preconditions: Vec<ProcedurePrecondition>,
    pub required_approvals: Vec<ApprovalKind>,
    pub steps: Vec<ProcedureStep>,
    pub write_capable: bool,
}

impl ProcedureDefinition {
    #[must_use]
    pub fn validate_for_session(&self, session: &AutomotiveSession) -> Vec<String> {
        let mut failures = Vec::new();

        for precondition in &self.preconditions {
            match precondition {
                ProcedurePrecondition::ToolAttached if session.active_adapter_id.is_none() => {
                    failures.push("procedure requires an attached adapter".to_string());
                }
                ProcedurePrecondition::VehicleIdentified if session.vehicle_identity.is_none() => {
                    failures.push("procedure requires identified vehicle identity".to_string());
                }
                ProcedurePrecondition::EcuIdentified if session.ecu_identity.is_none() => {
                    failures.push("procedure requires identified ECU".to_string());
                }
                ProcedurePrecondition::IgnitionOn
                    if session.ignition_state != IgnitionState::On =>
                {
                    failures.push("procedure requires ignition state ON".to_string());
                }
                ProcedurePrecondition::StableBattery
                    if session.battery_state != BatteryState::Stable =>
                {
                    failures.push("procedure requires stable battery".to_string());
                }
                ProcedurePrecondition::SafeCheckpointCaptured
                    if session.last_safe_checkpoint.is_none() =>
                {
                    failures.push("procedure requires a safe checkpoint".to_string());
                }
                _ => {}
            }
        }

        failures
    }
}

#[cfg(test)]
mod tests {
    use super::{ProcedureDefinition, ProcedurePrecondition, ProcedureStep, ProcedureStepKind};
    use crate::adapters::{AdapterOperation, ToolFamily};
    use crate::sessions::{
        ActionClass, AutomotiveSession, BatteryState, EcuIdentity, IgnitionState, SafeCheckpoint,
        SessionMode, VehicleIdentity,
    };

    fn write_capable_procedure() -> ProcedureDefinition {
        ProcedureDefinition {
            procedure_id: "renault-key-relearn".to_string(),
            display_name: "Renault key relearn".to_string(),
            required_tool: ToolFamily::Renolink,
            required_ecu_family: Some("UCH".to_string()),
            preconditions: vec![
                ProcedurePrecondition::ToolAttached,
                ProcedurePrecondition::VehicleIdentified,
                ProcedurePrecondition::EcuIdentified,
                ProcedurePrecondition::IgnitionOn,
                ProcedurePrecondition::StableBattery,
                ProcedurePrecondition::SafeCheckpointCaptured,
            ],
            required_approvals: vec![crate::sessions::ApprovalKind::ManualWrite],
            steps: vec![
                ProcedureStep {
                    step_id: "step-1".to_string(),
                    kind: ProcedureStepKind::SafeRead,
                    summary: "capture original key slots".to_string(),
                    tool_family: Some(ToolFamily::Renolink),
                    adapter_operation: Some(AdapterOperation::ReadDtcs),
                    action_class: Some(ActionClass::Read),
                },
                ProcedureStep {
                    step_id: "step-2".to_string(),
                    kind: ProcedureStepKind::ExecuteWrite,
                    summary: "program replacement key".to_string(),
                    tool_family: Some(ToolFamily::Renolink),
                    adapter_operation: Some(AdapterOperation::ExecuteWrite),
                    action_class: Some(ActionClass::Write),
                },
            ],
            write_capable: true,
        }
    }

    fn ready_session() -> AutomotiveSession {
        AutomotiveSession {
            session_id: "session-1".to_string(),
            host_machine: "diag-station-01".to_string(),
            active_adapter_id: Some("renolink-win11".to_string()),
            vehicle_identity: Some(VehicleIdentity {
                vin: "VF1ABCDEFG".to_string(),
                brand: "Renault".to_string(),
                model: "Clio".to_string(),
                year: Some(2016),
            }),
            ecu_identity: Some(EcuIdentity {
                address: "0x7a".to_string(),
                family: "UCH".to_string(),
                software_version: Some("3.4".to_string()),
            }),
            ignition_state: IgnitionState::On,
            battery_state: BatteryState::Stable,
            active_procedure_id: Some("renault-key-relearn".to_string()),
            current_mode: SessionMode::GuidedWrite,
            pending_approval: None,
            last_safe_checkpoint: Some(SafeCheckpoint {
                checkpoint_id: "safe-1".to_string(),
                summary: "baseline capture".to_string(),
                vehicle_vin: "VF1ABCDEFG".to_string(),
                ecu_address: Some("0x7a".to_string()),
            }),
            audit_trace_id: "trace-7".to_string(),
        }
    }

    #[test]
    fn write_procedure_accepts_a_ready_session() {
        let failures = write_capable_procedure().validate_for_session(&ready_session());
        assert!(failures.is_empty());
    }

    #[test]
    fn missing_checkpoint_blocks_the_procedure() {
        let mut session = ready_session();
        session.last_safe_checkpoint = None;

        let failures = write_capable_procedure().validate_for_session(&session);
        assert!(failures.contains(&"procedure requires a safe checkpoint".to_string()));
    }
}
