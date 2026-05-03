use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::schemas::{HostRequestEnvelope, HostResponseEnvelope, LocalTransport};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeMode {
    PluginSubprocess,
    McpStdio,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeDescriptor {
    pub mode: BridgeMode,
    pub stateless: bool,
    pub stdin_json: bool,
    pub stdout_json: bool,
    pub host_transport: LocalTransport,
    pub request_tool_name: String,
    pub zero_trust_notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThinBridgeContract {
    pub descriptors: Vec<BridgeDescriptor>,
}

impl ThinBridgeContract {
    #[must_use]
    pub fn new(host_transport: LocalTransport) -> Self {
        Self {
            descriptors: vec![
                BridgeDescriptor {
                    mode: BridgeMode::PluginSubprocess,
                    stateless: true,
                    stdin_json: true,
                    stdout_json: true,
                    host_transport: host_transport.clone(),
                    request_tool_name: "automotive_host.dispatch".to_string(),
                    zero_trust_notes: vec![
                        "plugin bridge only translates stdin JSON into host requests".to_string(),
                        "no vendor-specific business logic is allowed in the bridge".to_string(),
                    ],
                },
                BridgeDescriptor {
                    mode: BridgeMode::McpStdio,
                    stateless: true,
                    stdin_json: true,
                    stdout_json: true,
                    host_transport,
                    request_tool_name: "automotive_host.dispatch".to_string(),
                    zero_trust_notes: vec![
                        "MCP bridge keeps the same request envelope as plugin mode".to_string(),
                        "tool invocation stays local-only through the host API boundary"
                            .to_string(),
                    ],
                },
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeRequestEnvelope {
    pub mode: BridgeMode,
    pub request: HostRequestEnvelope,
}

impl BridgeRequestEnvelope {
    #[must_use]
    pub fn as_plugin_payload(&self) -> Value {
        json!({
            "mode": self.mode,
            "request": self.request,
        })
    }

    #[must_use]
    pub fn as_mcp_tool_call(&self) -> Value {
        json!({
            "name": "automotive_host.dispatch",
            "arguments": {
                "request": self.request,
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BridgeResponseEnvelope {
    pub mode: BridgeMode,
    pub response: HostResponseEnvelope,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{BridgeMode, BridgeRequestEnvelope, ThinBridgeContract};
    use crate::schemas::{
        ApiGroup, ExecuteApprovedActionRequest, HostRequestEnvelope, OperationName, RequestPayload,
    };
    use crate::sessions::ActionClass;

    fn sample_request() -> HostRequestEnvelope {
        HostRequestEnvelope {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            api_group: ApiGroup::Execute,
            action: OperationName::ExecuteApprovedAction,
            payload: RequestPayload::ExecuteApprovedAction(ExecuteApprovedActionRequest {
                session_id: "session-1".to_string(),
                approval_id: "approval-1".to_string(),
                action_class: ActionClass::Write,
            }),
        }
    }

    #[test]
    fn bridge_contract_exposes_plugin_and_mcp_modes() {
        let contract = ThinBridgeContract::new(crate::schemas::LocalTransport::default());
        assert_eq!(contract.descriptors.len(), 2);
        assert_eq!(
            contract.descriptors[0].request_tool_name,
            "automotive_host.dispatch"
        );
    }

    #[test]
    fn plugin_and_mcp_payloads_share_the_same_host_request() {
        let envelope = BridgeRequestEnvelope {
            mode: BridgeMode::PluginSubprocess,
            request: sample_request(),
        };

        let plugin_payload = envelope.as_plugin_payload();
        let mcp_payload = envelope.as_mcp_tool_call();

        assert_eq!(
            plugin_payload["request"]["request_id"],
            mcp_payload["arguments"]["request"]["request_id"]
        );
        assert_eq!(mcp_payload["name"], json!("automotive_host.dispatch"));
    }
}
