use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::adapters::{
    adapter_catalog, AdapterDescriptor, AdapterOperation, AdapterRegistry, WindowsVersion,
};
use crate::bridge::ThinBridgeContract;
use crate::schemas::{
    ApiGroup, HostRequestEnvelope, HostResponseEnvelope, HostResponseStatus, LocalTransport,
    OperationName, RequestPayload,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostModule {
    Host,
    Adapters,
    Procedures,
    Policy,
    Sessions,
    Audit,
    Schemas,
    Bridge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostConfig {
    pub host_id: String,
    pub transport: LocalTransport,
    pub supported_windows: Vec<WindowsVersion>,
    pub user_session_required: bool,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            host_id: "automotive-host".to_string(),
            transport: LocalTransport::default(),
            supported_windows: vec![WindowsVersion::Win7, WindowsVersion::Win11],
            user_session_required: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostBoundary {
    pub modules: Vec<HostModule>,
    pub api_groups: Vec<ApiGroup>,
    pub transport: LocalTransport,
    pub same_machine_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomotiveHostAgent {
    pub config: HostConfig,
    pub boundary: HostBoundary,
    pub adapters: Vec<AdapterDescriptor>,
    pub bridge: ThinBridgeContract,
}

impl AutomotiveHostAgent {
    #[must_use]
    pub fn new(config: HostConfig) -> Self {
        let transport = config.transport.clone();
        let adapters = adapter_catalog();
        let bridge = ThinBridgeContract::new(transport.clone());

        Self {
            config,
            boundary: HostBoundary {
                modules: vec![
                    HostModule::Host,
                    HostModule::Adapters,
                    HostModule::Procedures,
                    HostModule::Policy,
                    HostModule::Sessions,
                    HostModule::Audit,
                    HostModule::Schemas,
                    HostModule::Bridge,
                ],
                api_groups: vec![
                    ApiGroup::Session,
                    ApiGroup::Vehicle,
                    ApiGroup::Ecu,
                    ApiGroup::Dtc,
                    ApiGroup::Procedure,
                    ApiGroup::Approval,
                    ApiGroup::Execute,
                    ApiGroup::Verify,
                    ApiGroup::Audit,
                ],
                transport,
                same_machine_only: true,
            },
            adapters,
            bridge,
        }
    }

    #[must_use]
    pub fn preview_dispatch(&self, request: &HostRequestEnvelope) -> HostResponseEnvelope {
        if let Some(response) = self.preview_adapter_dispatch(request) {
            return response;
        }

        let module = match request.action {
            OperationName::LaunchTool | OperationName::DispatchAdapterOperation => "adapters",
            OperationName::StartSession
            | OperationName::AttachTool
            | OperationName::CloseSession => "sessions",
            OperationName::ReadFaults
            | OperationName::ReadDtcs
            | OperationName::ReadLiveData
            | OperationName::DiscoverVehicle
            | OperationName::IdentifyEcu => "adapters",
            OperationName::LoadProcedure | OperationName::SimulateProcedure => "procedures",
            OperationName::ExecuteApprovedAction | OperationName::ExecuteWrite => "policy",
            OperationName::VerifySession | OperationName::VerifyPostAction => "verify",
            OperationName::ConnectTool
            | OperationName::SimulateAction
            | OperationName::RequestApproval
            | OperationName::ExportReport => "host",
        };

        HostResponseEnvelope::completed(
            request.request_id.clone(),
            request.trace_id.clone(),
            format!("request routed to {module} boundary"),
            Some(json!({
                "hostId": self.config.host_id,
                "sameMachineOnly": self.boundary.same_machine_only,
                "transport": self.boundary.transport,
            })),
        )
    }

    fn preview_adapter_dispatch(
        &self,
        request: &HostRequestEnvelope,
    ) -> Option<HostResponseEnvelope> {
        let (tool, windows_version, operation) = match &request.payload {
            RequestPayload::LaunchTool(payload) => (
                payload.tool,
                payload.windows_version,
                AdapterOperation::ConnectTool,
            ),
            RequestPayload::AttachTool(payload) => (
                payload.tool,
                payload.windows_version,
                AdapterOperation::ConnectTool,
            ),
            RequestPayload::DispatchAdapterOperation(payload) => {
                (payload.tool, payload.windows_version, payload.operation)
            }
            _ => return None,
        };

        let registry = AdapterRegistry::default();
        let Some(adapter) = registry.resolve(tool, windows_version) else {
            return Some(HostResponseEnvelope {
                request_id: request.request_id.clone(),
                trace_id: request.trace_id.clone(),
                status: HostResponseStatus::Failed,
                summary: "adapter variant not registered".to_string(),
                data: Some(json!({
                    "tool": tool,
                    "windowsVersion": windows_version,
                })),
            });
        };

        Some(HostResponseEnvelope::completed(
            request.request_id.clone(),
            request.trace_id.clone(),
            format!(
                "adapter request routed to {}",
                adapter.descriptor().adapter_id
            ),
            Some(json!({
                "module": "adapters",
                "hostId": self.config.host_id,
                "transport": self.boundary.transport,
                "sameMachineOnly": self.boundary.same_machine_only,
                "adapter": adapter.descriptor(),
                "launchSpec": adapter.launch_spec(),
                "checkpoints": adapter.navigation_checkpoints(),
                "extractionTargets": adapter.extraction_targets(),
                "plan": adapter.plan(operation),
            })),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{AutomotiveHostAgent, HostConfig, HostModule};
    use serde_json::json;

    use crate::schemas::{
        AdapterActionRequest, ApiGroup, HostRequestEnvelope, HostResponseStatus, OperationName,
        RequestPayload, StartSessionRequest,
    };
    use crate::{AdapterSupportLevel, ToolFamily, WindowsVersion};

    #[test]
    fn host_agent_exposes_all_domain_modules() {
        let host = AutomotiveHostAgent::new(HostConfig::default());

        assert!(host.boundary.modules.contains(&HostModule::Adapters));
        assert!(host.boundary.modules.contains(&HostModule::Policy));
        assert!(host.boundary.api_groups.contains(&ApiGroup::Audit));
        assert!(host.boundary.same_machine_only);
        assert_eq!(host.adapters.len(), 8);
    }

    #[test]
    fn dispatch_preview_keeps_requests_inside_local_boundary() {
        let host = AutomotiveHostAgent::new(HostConfig::default());
        let response = host.preview_dispatch(&HostRequestEnvelope {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            api_group: ApiGroup::Session,
            action: OperationName::StartSession,
            payload: RequestPayload::StartSession(StartSessionRequest {
                session_id: "session-1".to_string(),
                host_machine: "ws-11".to_string(),
                operator_id: "operator-7".to_string(),
                trace_id: "trace-1".to_string(),
            }),
        });

        assert!(response.summary.contains("sessions"));
    }

    #[test]
    fn adapter_dispatch_resolves_registered_variant_and_returns_plan() {
        let host = AutomotiveHostAgent::new(HostConfig::default());
        let response = host.preview_dispatch(&HostRequestEnvelope {
            request_id: "req-2".to_string(),
            trace_id: "trace-2".to_string(),
            api_group: ApiGroup::Execute,
            action: OperationName::DispatchAdapterOperation,
            payload: RequestPayload::DispatchAdapterOperation(AdapterActionRequest {
                session_id: "session-2".to_string(),
                tool: ToolFamily::OpCom,
                windows_version: WindowsVersion::Win11,
                operation: crate::AdapterOperation::ReadDtcs,
                target_ecu_id: Some("0x10".to_string()),
                procedure_id: None,
            }),
        });

        assert_eq!(response.status, HostResponseStatus::Completed);
        assert_eq!(
            response.data.as_ref().expect("adapter data")["adapter"]["support_level"],
            json!(AdapterSupportLevel::Supported)
        );
        assert_eq!(
            response.data.as_ref().expect("adapter data")["plan"]["supported"],
            json!(true)
        );
    }
}
