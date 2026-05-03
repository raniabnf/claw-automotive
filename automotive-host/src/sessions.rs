use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionClass {
    Read,
    Write,
    Flash,
    Coding,
}

impl ActionClass {
    #[must_use]
    pub fn is_irreversible(self) -> bool {
        matches!(self, Self::Write | Self::Flash | Self::Coding)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    ReadOnly,
    GuidedWrite,
    Coding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IgnitionState {
    Unknown,
    Off,
    Accessory,
    On,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "millivolts", rename_all = "snake_case")]
pub enum BatteryState {
    Unknown,
    Low,
    Stable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKind {
    OperatorPresence,
    ManualWrite,
    FlashProgramming,
    CodingChange,
    EnvironmentTrust,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    NotRequired,
    Pending,
    Approved,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VehicleIdentity {
    pub vin: String,
    pub brand: String,
    pub model: String,
    pub year: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcuIdentity {
    pub address: String,
    pub family: String,
    pub software_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafeCheckpoint {
    pub checkpoint_id: String,
    pub summary: String,
    pub vehicle_vin: String,
    pub ecu_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalTicket {
    pub approval_id: String,
    pub kind: ApprovalKind,
    pub status: ApprovalStatus,
    pub reason: String,
    pub requested_by: String,
    pub approved_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomotiveSession {
    pub session_id: String,
    pub host_machine: String,
    pub active_adapter_id: Option<String>,
    pub vehicle_identity: Option<VehicleIdentity>,
    pub ecu_identity: Option<EcuIdentity>,
    pub ignition_state: IgnitionState,
    pub battery_state: BatteryState,
    pub active_procedure_id: Option<String>,
    pub current_mode: SessionMode,
    pub pending_approval: Option<ApprovalTicket>,
    pub last_safe_checkpoint: Option<SafeCheckpoint>,
    pub audit_trace_id: String,
}

impl AutomotiveSession {
    #[must_use]
    pub fn is_write_ready(&self) -> bool {
        self.active_adapter_id.is_some()
            && self.vehicle_identity.is_some()
            && self.ecu_identity.is_some()
            && self.active_procedure_id.is_some()
            && self.ignition_state == IgnitionState::On
            && self.battery_state == BatteryState::Stable
    }

    #[must_use]
    pub fn approval_open(&self) -> bool {
        self.pending_approval
            .as_ref()
            .is_some_and(|ticket| ticket.status == ApprovalStatus::Pending)
    }

    #[must_use]
    pub fn approval_required_for(&self, action_class: ActionClass) -> bool {
        action_class.is_irreversible()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActionClass, ApprovalKind, ApprovalStatus, ApprovalTicket, AutomotiveSession, BatteryState,
        EcuIdentity, IgnitionState, SafeCheckpoint, SessionMode, VehicleIdentity,
    };

    fn sample_session() -> AutomotiveSession {
        AutomotiveSession {
            session_id: "session-1".to_string(),
            host_machine: "diag-station-01".to_string(),
            active_adapter_id: Some("renolink-win11".to_string()),
            vehicle_identity: Some(VehicleIdentity {
                vin: "VF1AAAAA654321".to_string(),
                brand: "Renault".to_string(),
                model: "Clio".to_string(),
                year: Some(2018),
            }),
            ecu_identity: Some(EcuIdentity {
                address: "0x7a".to_string(),
                family: "UCH".to_string(),
                software_version: Some("1.0.3".to_string()),
            }),
            ignition_state: IgnitionState::On,
            battery_state: BatteryState::Stable,
            active_procedure_id: Some("key-relearn".to_string()),
            current_mode: SessionMode::GuidedWrite,
            pending_approval: Some(ApprovalTicket {
                approval_id: "approval-1".to_string(),
                kind: ApprovalKind::ManualWrite,
                status: ApprovalStatus::Pending,
                reason: "coding key slot 2".to_string(),
                requested_by: "operator-1".to_string(),
                approved_by: None,
            }),
            last_safe_checkpoint: Some(SafeCheckpoint {
                checkpoint_id: "safe-1".to_string(),
                summary: "baseline dump captured".to_string(),
                vehicle_vin: "VF1AAAAA654321".to_string(),
                ecu_address: Some("0x7a".to_string()),
            }),
            audit_trace_id: "trace-1".to_string(),
        }
    }

    #[test]
    fn write_ready_requires_vehicle_adapter_and_checkpoint_inputs() {
        let session = sample_session();
        assert!(session.is_write_ready());
    }

    #[test]
    fn pending_ticket_is_reported_as_open_approval() {
        let session = sample_session();
        assert!(session.approval_open());
        assert!(session.approval_required_for(ActionClass::Write));
    }

    #[test]
    fn read_only_session_does_not_require_read_approval() {
        let mut session = sample_session();
        session.current_mode = SessionMode::ReadOnly;
        assert!(!session.approval_required_for(ActionClass::Read));
        assert!(session.approval_required_for(ActionClass::Flash));
    }
}
