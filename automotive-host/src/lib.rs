pub mod adapters;
pub mod audit;
pub mod bridge;
pub mod host;
pub mod policy;
pub mod procedures;
pub mod schemas;
pub mod sessions;

pub use adapters::{
    adapter_catalog, AdapterCapability, AdapterDescriptor, AdapterFailureKind,
    AdapterInvocationPlan, AdapterOperation, AdapterPhase, AdapterRegistry, AdapterSupportLevel,
    DtcRecord, ExportArtifact, ExtractionKind, ExtractionTarget, LiveDataPoint,
    NavigationCheckpoint, NormalizedAdapterOutput, ToolFamily, ToolHealth, ToolLaunchSpec,
    VehicleToolAdapter, WindowIdentity, WindowsVersion,
};
pub use bridge::{
    BridgeDescriptor, BridgeMode, BridgeRequestEnvelope, BridgeResponseEnvelope, ThinBridgeContract,
};
pub use host::{AutomotiveHostAgent, HostBoundary, HostConfig, HostModule};
pub use policy::{ActionRequestContext, OperatorContext, PolicyDecision, PolicyEngine, TrustLevel};
pub use procedures::{ProcedureDefinition, ProcedurePrecondition, ProcedureStep};
pub use schemas::{
    AdapterActionRequest, ApiGroup, AttachToolRequest, CloseSessionRequest,
    ExecuteApprovedActionRequest, HostRequestEnvelope, HostResponseEnvelope, HostResponseStatus,
    LaunchToolRequest, LoadProcedureRequest, LocalTransport, OperationName, ReadFaultsRequest,
    RequestPayload, SimulateProcedureRequest, StartSessionRequest, VerifySessionRequest,
};
pub use sessions::{
    ActionClass, ApprovalKind, ApprovalStatus, ApprovalTicket, AutomotiveSession, BatteryState,
    EcuIdentity, IgnitionState, SafeCheckpoint, SessionMode, VehicleIdentity,
};
