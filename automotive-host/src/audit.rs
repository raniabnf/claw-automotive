use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventKind {
    SessionOpened,
    ActionRequested,
    ApprovalRequested,
    ApprovalResolved,
    ActionExecuted,
    VerificationRecorded,
    SessionClosed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub trace_id: String,
    pub session_id: String,
    pub event_kind: AuditEventKind,
    pub detail: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditStore {
    records: Vec<AuditRecord>,
}

impl AuditStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, record: AuditRecord) {
        self.records.push(record);
    }

    #[must_use]
    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }
}

#[cfg(test)]
mod tests {
    use super::{AuditEventKind, AuditRecord, AuditStore};

    #[test]
    fn audit_store_is_append_only() {
        let mut store = AuditStore::new();
        store.append(AuditRecord {
            trace_id: "trace-1".to_string(),
            session_id: "session-1".to_string(),
            event_kind: AuditEventKind::SessionOpened,
            detail: "session started".to_string(),
            evidence_refs: vec!["vehicle-photo.png".to_string()],
        });
        store.append(AuditRecord {
            trace_id: "trace-1".to_string(),
            session_id: "session-1".to_string(),
            event_kind: AuditEventKind::ApprovalRequested,
            detail: "requesting operator confirmation".to_string(),
            evidence_refs: vec!["precheck.json".to_string()],
        });

        assert_eq!(store.records().len(), 2);
        assert_eq!(store.records()[0].event_kind, AuditEventKind::SessionOpened);
        assert_eq!(
            store.records()[1].event_kind,
            AuditEventKind::ApprovalRequested
        );
    }
}
