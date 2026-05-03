use automotive_host::{
    ActionClass, ActionRequestContext, AdapterOperation, AdapterRegistry, AdapterSupportLevel,
    ApiGroup, AutomotiveHostAgent, BridgeMode, BridgeRequestEnvelope, HostConfig,
    HostRequestEnvelope, OperationName, OperatorContext, PolicyDecision, PolicyEngine,
    RequestPayload, SessionMode, StartSessionRequest, ToolFamily, TrustLevel, WindowsVersion,
};

#[test]
fn host_contract_stays_same_machine_and_bridge_ready() {
    let host = AutomotiveHostAgent::new(HostConfig::default());

    assert!(host.boundary.same_machine_only);
    assert_eq!(host.bridge.descriptors.len(), 2);
    assert!(host
        .bridge
        .descriptors
        .iter()
        .all(|descriptor| descriptor.request_tool_name == "automotive_host.dispatch"));
}

#[test]
fn bridge_request_can_wrap_a_host_request_without_rewriting_it() {
    let request = HostRequestEnvelope {
        request_id: "req-22".to_string(),
        trace_id: "trace-22".to_string(),
        api_group: ApiGroup::Session,
        action: OperationName::StartSession,
        payload: RequestPayload::StartSession(StartSessionRequest {
            session_id: "session-22".to_string(),
            host_machine: "win11-station".to_string(),
            operator_id: "operator-22".to_string(),
            trace_id: "trace-22".to_string(),
        }),
    };

    let envelope = BridgeRequestEnvelope {
        mode: BridgeMode::PluginSubprocess,
        request: request.clone(),
    };

    assert_eq!(envelope.request, request);
    assert_eq!(
        envelope.as_mcp_tool_call()["arguments"]["request"]["trace_id"],
        "trace-22"
    );
}

#[test]
fn write_ready_sessions_still_require_policy_approval() {
    let session = automotive_host::AutomotiveSession {
        session_id: "session-9".to_string(),
        host_machine: "win7-bay".to_string(),
        active_adapter_id: Some("opcom-win7".to_string()),
        vehicle_identity: Some(automotive_host::VehicleIdentity {
            vin: "W0L000000000".to_string(),
            brand: "Opel".to_string(),
            model: "Astra".to_string(),
            year: Some(2012),
        }),
        ecu_identity: Some(automotive_host::EcuIdentity {
            address: "0x40".to_string(),
            family: "BCM".to_string(),
            software_version: Some("2.1".to_string()),
        }),
        ignition_state: automotive_host::IgnitionState::On,
        battery_state: automotive_host::BatteryState::Stable,
        active_procedure_id: Some("bcm-coding".to_string()),
        current_mode: SessionMode::GuidedWrite,
        pending_approval: None,
        last_safe_checkpoint: Some(automotive_host::SafeCheckpoint {
            checkpoint_id: "safe-9".to_string(),
            summary: "baseline export".to_string(),
            vehicle_vin: "W0L000000000".to_string(),
            ecu_address: Some("0x40".to_string()),
        }),
        audit_trace_id: "trace-9".to_string(),
    };
    let engine = PolicyEngine::new();
    let decision = engine.evaluate(
        &session,
        &OperatorContext {
            operator_id: "operator-9".to_string(),
            is_physically_present: true,
            trust_level: TrustLevel::VerifiedLocalOperator,
        },
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
fn registry_keeps_opcom_supported_and_other_tool_scaffolds_planned() {
    let registry = AdapterRegistry::default();
    let opcom = registry
        .resolve(ToolFamily::OpCom, WindowsVersion::Win11)
        .expect("opcom variant");
    let renolink = registry
        .resolve(ToolFamily::Renolink, WindowsVersion::Win11)
        .expect("renolink variant");

    assert_eq!(
        opcom.descriptor().support_level,
        AdapterSupportLevel::Supported
    );
    assert!(opcom.plan(AdapterOperation::ReadDtcs).supported);
    assert_eq!(
        renolink.descriptor().support_level,
        AdapterSupportLevel::Planned
    );
    assert!(!renolink.plan(AdapterOperation::ReadDtcs).supported);
}
