use serde::{Deserialize, Serialize};

use crate::sessions::{ActionClass, ApprovalKind, AutomotiveSession, BatteryState, IgnitionState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    VerifiedLocalOperator,
    VerifiedRemoteOperator,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorContext {
    pub operator_id: String,
    pub is_physically_present: bool,
    pub trust_level: TrustLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Policy inputs mirror procedural checklist until H0 refactor
pub struct ActionRequestContext {
    pub action_class: ActionClass,
    pub adapter_connected: bool,
    pub procedure_loaded: bool,
    pub target_matches_session: bool,
    pub last_checkpoint_present: bool,
    pub simulation_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum PolicyDecision {
    Allowed {
        reasons: Vec<String>,
    },
    RequiresApproval {
        reasons: Vec<String>,
        approvals: Vec<ApprovalKind>,
    },
    Denied {
        reasons: Vec<String>,
    },
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PolicyEngine;

impl PolicyEngine {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn evaluate(
        &self,
        session: &AutomotiveSession,
        operator: &OperatorContext,
        action: &ActionRequestContext,
    ) -> PolicyDecision {
        let mut deny_reasons = Vec::new();
        let mut approval_reasons = Vec::new();
        let mut approvals = Vec::new();

        if !action.adapter_connected {
            deny_reasons.push("adapter must be connected through a typed host session".to_string());
        }
        if !action.target_matches_session {
            deny_reasons
                .push("target vehicle or ECU does not match the active session".to_string());
        }
        if action.action_class.is_irreversible() && !action.procedure_loaded {
            deny_reasons
                .push("write-capable actions require a loaded approved procedure".to_string());
        }
        if action.action_class.is_irreversible() && !action.last_checkpoint_present {
            deny_reasons.push(
                "write-capable actions require a safe checkpoint before execution".to_string(),
            );
        }
        if action.action_class.is_irreversible() && session.ignition_state != IgnitionState::On {
            deny_reasons.push("write-capable actions require ignition state ON".to_string());
        }
        if action.action_class.is_irreversible() && session.battery_state != BatteryState::Stable {
            deny_reasons.push("write-capable actions require stable battery state".to_string());
        }
        if action.action_class.is_irreversible() && !session.is_write_ready() {
            deny_reasons.push("session is not write-ready".to_string());
        }

        if operator.trust_level == TrustLevel::Unverified {
            if action.action_class.is_irreversible() {
                deny_reasons
                    .push("unverified operators cannot execute irreversible actions".to_string());
            } else {
                approval_reasons.push(
                    "operator identity must be verified before non-destructive work".to_string(),
                );
                approvals.push(ApprovalKind::EnvironmentTrust);
            }
        }

        if matches!(
            action.action_class,
            ActionClass::Write | ActionClass::Flash | ActionClass::Coding
        ) {
            approval_reasons.push(
                "sensitive automotive actions require explicit operator approval".to_string(),
            );
            approvals.push(match action.action_class {
                ActionClass::Write => ApprovalKind::ManualWrite,
                ActionClass::Flash => ApprovalKind::FlashProgramming,
                ActionClass::Coding => ApprovalKind::CodingChange,
                ActionClass::Read => ApprovalKind::OperatorPresence,
            });
        }

        if matches!(operator.trust_level, TrustLevel::VerifiedRemoteOperator)
            && action.action_class.is_irreversible()
        {
            approval_reasons.push(
                "remote verified operators still require local operator presence for writes"
                    .to_string(),
            );
            approvals.push(ApprovalKind::OperatorPresence);
        }

        if action.simulation_only {
            approval_reasons.retain(|reason| {
                reason != "sensitive automotive actions require explicit operator approval"
            });
            approvals.retain(|approval| *approval != ApprovalKind::ManualWrite);
        }

        dedupe(&mut approvals);
        dedupe(&mut approval_reasons);

        if !deny_reasons.is_empty() {
            return PolicyDecision::Denied {
                reasons: deny_reasons,
            };
        }

        if !approvals.is_empty() || session.approval_required_for(action.action_class) {
            return PolicyDecision::RequiresApproval {
                reasons: approval_reasons,
                approvals,
            };
        }

        PolicyDecision::Allowed {
            reasons: vec!["request stays inside the typed local host boundary".to_string()],
        }
    }
}

fn dedupe<T: PartialEq>(items: &mut Vec<T>) {
    let mut index = 0;
    while index < items.len() {
        if items[..index].contains(&items[index]) {
            items.remove(index);
        } else {
            index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ActionRequestContext, OperatorContext, PolicyDecision, PolicyEngine, TrustLevel};
    use crate::sessions::{
        ActionClass, AutomotiveSession, BatteryState, EcuIdentity, IgnitionState, SessionMode,
        VehicleIdentity,
    };

    fn sample_session() -> AutomotiveSession {
        AutomotiveSession {
            session_id: "session-1".to_string(),
            host_machine: "ws-11".to_string(),
            active_adapter_id: Some("xentry-win11".to_string()),
            vehicle_identity: Some(VehicleIdentity {
                vin: "WDD123456789".to_string(),
                brand: "Mercedes".to_string(),
                model: "W204".to_string(),
                year: Some(2014),
            }),
            ecu_identity: Some(EcuIdentity {
                address: "0x10".to_string(),
                family: "ECM".to_string(),
                software_version: Some("24.7".to_string()),
            }),
            ignition_state: IgnitionState::On,
            battery_state: BatteryState::Stable,
            active_procedure_id: Some("injector-coding".to_string()),
            current_mode: SessionMode::GuidedWrite,
            pending_approval: None,
            last_safe_checkpoint: None,
            audit_trace_id: "trace-1".to_string(),
        }
    }

    fn local_operator() -> OperatorContext {
        OperatorContext {
            operator_id: "operator-7".to_string(),
            is_physically_present: true,
            trust_level: TrustLevel::VerifiedLocalOperator,
        }
    }

    #[test]
    fn read_actions_can_run_without_manual_write_approval() {
        let engine = PolicyEngine::new();
        let decision = engine.evaluate(
            &sample_session(),
            &local_operator(),
            &ActionRequestContext {
                action_class: ActionClass::Read,
                adapter_connected: true,
                procedure_loaded: true,
                target_matches_session: true,
                last_checkpoint_present: true,
                simulation_only: false,
            },
        );

        assert!(matches!(decision, PolicyDecision::Allowed { .. }));
    }

    #[test]
    fn write_actions_require_explicit_approval() {
        let engine = PolicyEngine::new();
        let mut session = sample_session();
        session.last_safe_checkpoint = Some(crate::sessions::SafeCheckpoint {
            checkpoint_id: "safe-2".to_string(),
            summary: "coding baseline".to_string(),
            vehicle_vin: "WDD123456789".to_string(),
            ecu_address: Some("0x10".to_string()),
        });

        let decision = engine.evaluate(
            &session,
            &local_operator(),
            &ActionRequestContext {
                action_class: ActionClass::Write,
                adapter_connected: true,
                procedure_loaded: true,
                target_matches_session: true,
                last_checkpoint_present: true,
                simulation_only: false,
            },
        );

        assert!(matches!(decision, PolicyDecision::RequiresApproval { .. }));
    }

    #[test]
    fn unverified_operator_is_denied_for_irreversible_actions() {
        let engine = PolicyEngine::new();
        let mut session = sample_session();
        session.last_safe_checkpoint = Some(crate::sessions::SafeCheckpoint {
            checkpoint_id: "safe-3".to_string(),
            summary: "flash baseline".to_string(),
            vehicle_vin: "WDD123456789".to_string(),
            ecu_address: Some("0x10".to_string()),
        });
        let operator = OperatorContext {
            operator_id: "unknown".to_string(),
            is_physically_present: false,
            trust_level: TrustLevel::Unverified,
        };

        let decision = engine.evaluate(
            &session,
            &operator,
            &ActionRequestContext {
                action_class: ActionClass::Flash,
                adapter_connected: true,
                procedure_loaded: true,
                target_matches_session: true,
                last_checkpoint_present: true,
                simulation_only: false,
            },
        );

        assert!(matches!(decision, PolicyDecision::Denied { .. }));
    }
}
