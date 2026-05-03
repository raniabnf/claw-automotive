use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::adapters::{AdapterOperation, ToolFamily, WindowsVersion};
use crate::sessions::ActionClass;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LocalTransport {
    LoopbackHttp { bind: String, port: u16 },
    NamedPipe { pipe_name: String },
}

impl Default for LocalTransport {
    fn default() -> Self {
        Self::LoopbackHttp {
            bind: "127.0.0.1".to_string(),
            port: 8847,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiGroup {
    Session,
    Vehicle,
    Ecu,
    Dtc,
    Procedure,
    Approval,
    Execute,
    Verify,
    Audit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationName {
    LaunchTool,
    ConnectTool,
    DiscoverVehicle,
    IdentifyEcu,
    ReadDtcs,
    ReadLiveData,
    LoadProcedure,
    SimulateAction,
    RequestApproval,
    ExecuteWrite,
    VerifyPostAction,
    ExportReport,
    DispatchAdapterOperation,
    StartSession,
    AttachTool,
    ReadFaults,
    SimulateProcedure,
    ExecuteApprovedAction,
    VerifySession,
    CloseSession,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartSessionRequest {
    pub session_id: String,
    pub host_machine: String,
    pub operator_id: String,
    pub trace_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchToolRequest {
    pub session_id: String,
    pub tool: ToolFamily,
    pub windows_version: WindowsVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachToolRequest {
    pub session_id: String,
    pub tool: ToolFamily,
    pub windows_version: WindowsVersion,
    pub adapter_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterActionRequest {
    pub session_id: String,
    pub tool: ToolFamily,
    pub windows_version: WindowsVersion,
    pub operation: AdapterOperation,
    pub target_ecu_id: Option<String>,
    pub procedure_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadFaultsRequest {
    pub session_id: String,
    pub ecu_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadProcedureRequest {
    pub session_id: String,
    pub procedure_id: String,
    pub expected_tool: ToolFamily,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulateProcedureRequest {
    pub session_id: String,
    pub procedure_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecuteApprovedActionRequest {
    pub session_id: String,
    pub approval_id: String,
    pub action_class: ActionClass,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifySessionRequest {
    pub session_id: String,
    pub checkpoint_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseSessionRequest {
    pub session_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RequestPayload {
    LaunchTool(LaunchToolRequest),
    StartSession(StartSessionRequest),
    AttachTool(AttachToolRequest),
    DispatchAdapterOperation(AdapterActionRequest),
    ReadFaults(ReadFaultsRequest),
    LoadProcedure(LoadProcedureRequest),
    SimulateProcedure(SimulateProcedureRequest),
    ExecuteApprovedAction(ExecuteApprovedActionRequest),
    VerifySession(VerifySessionRequest),
    CloseSession(CloseSessionRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostRequestEnvelope {
    pub request_id: String,
    pub trace_id: String,
    pub api_group: ApiGroup,
    pub action: OperationName,
    pub payload: RequestPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostResponseStatus {
    Accepted,
    Completed,
    RequiresApproval,
    Denied,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostResponseEnvelope {
    pub request_id: String,
    pub trace_id: String,
    pub status: HostResponseStatus,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl HostResponseEnvelope {
    #[must_use]
    pub fn completed(
        request_id: impl Into<String>,
        trace_id: impl Into<String>,
        summary: impl Into<String>,
        data: Option<Value>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            trace_id: trace_id.into(),
            status: HostResponseStatus::Completed,
            summary: summary.into(),
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        AdapterActionRequest, ApiGroup, HostRequestEnvelope, HostResponseEnvelope,
        HostResponseStatus, OperationName, RequestPayload,
    };
    use crate::adapters::{AdapterOperation, ToolFamily, WindowsVersion};

    #[test]
    fn local_transport_defaults_to_loopback_http() {
        let transport = super::LocalTransport::default();
        let value = serde_json::to_value(transport).expect("transport serializes");

        assert_eq!(
            value,
            json!({
                "type": "loopback_http",
                "bind": "127.0.0.1",
                "port": 8847
            })
        );
    }

    #[test]
    fn request_envelope_carries_group_and_operation() {
        let request = HostRequestEnvelope {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            api_group: ApiGroup::Execute,
            action: OperationName::DispatchAdapterOperation,
            payload: RequestPayload::DispatchAdapterOperation(AdapterActionRequest {
                session_id: "session-1".to_string(),
                tool: ToolFamily::OpCom,
                windows_version: WindowsVersion::Win11,
                operation: AdapterOperation::ReadDtcs,
                target_ecu_id: Some("0x10".to_string()),
                procedure_id: None,
            }),
        };

        let value = serde_json::to_value(request).expect("request serializes");
        assert_eq!(value["api_group"], json!("execute"));
        assert_eq!(value["action"], json!("dispatch_adapter_operation"));
        assert_eq!(
            value["payload"]["kind"],
            json!("dispatch_adapter_operation")
        );
    }

    #[test]
    fn completed_response_preserves_optional_data() {
        let response = HostResponseEnvelope::completed(
            "req-9",
            "trace-9",
            "verification complete",
            Some(json!({ "checkpoint": "safe-1" })),
        );

        assert_eq!(response.status, HostResponseStatus::Completed);
        assert_eq!(
            serde_json::to_value(response).expect("response serializes")["data"]["checkpoint"],
            json!("safe-1")
        );
    }
}
