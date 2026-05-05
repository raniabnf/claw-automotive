mod common;
mod lexia;
mod opcom;
mod renolink;
mod xentry;

use serde::{Deserialize, Serialize};

pub use common::{
    AdapterFailureKind, AdapterInvocationPlan, AdapterPhase, AdapterSupportLevel, DtcRecord,
    ExportArtifact, ExtractionKind, ExtractionTarget, LiveDataPoint, NavigationCheckpoint,
    NormalizedAdapterOutput, ToolHealth, ToolLaunchSpec, WindowIdentity,
};
pub use lexia::LexiaAdapter;
pub use opcom::OpComAdapter;
pub use renolink::RenolinkAdapter;
pub use xentry::XentryAdapter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowsVersion {
    Win7,
    Win11,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolFamily {
    OpCom,
    Renolink,
    Lexia,
    Xentry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterCapability {
    ConnectTool,
    DiscoverVehicle,
    IdentifyEcu,
    ReadDtcs,
    ReadLiveData,
    LoadProcedure,
    SimulateAction,
    ExecuteWrite,
    VerifyPostAction,
    ExportReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterOperation {
    ConnectTool,
    DiscoverVehicle,
    IdentifyEcu,
    ReadDtcs,
    ReadLiveData,
    LoadProcedure,
    SimulateAction,
    ExecuteWrite,
    VerifyPostAction,
    ExportReport,
}

impl AdapterOperation {
    pub(crate) fn as_capability(self) -> AdapterCapability {
        match self {
            Self::ConnectTool => AdapterCapability::ConnectTool,
            Self::DiscoverVehicle => AdapterCapability::DiscoverVehicle,
            Self::IdentifyEcu => AdapterCapability::IdentifyEcu,
            Self::ReadDtcs => AdapterCapability::ReadDtcs,
            Self::ReadLiveData => AdapterCapability::ReadLiveData,
            Self::LoadProcedure => AdapterCapability::LoadProcedure,
            Self::SimulateAction => AdapterCapability::SimulateAction,
            Self::ExecuteWrite => AdapterCapability::ExecuteWrite,
            Self::VerifyPostAction => AdapterCapability::VerifyPostAction,
            Self::ExportReport => AdapterCapability::ExportReport,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterDescriptor {
    pub adapter_id: String,
    pub tool_family: ToolFamily,
    pub windows_version: WindowsVersion,
    pub support_level: AdapterSupportLevel,
    pub user_session_required: bool,
    pub local_only: bool,
    pub capabilities: Vec<AdapterCapability>,
    pub execution_notes: Vec<String>,
}

pub trait VehicleToolAdapter {
    fn descriptor(&self) -> &AdapterDescriptor;
    fn launch_spec(&self) -> &ToolLaunchSpec;
    fn navigation_checkpoints(&self) -> &[NavigationCheckpoint];
    fn extraction_targets(&self) -> &[ExtractionTarget];

    fn supports(&self, operation: AdapterOperation) -> bool {
        self.descriptor()
            .capabilities
            .contains(&operation.as_capability())
    }

    fn plan(&self, operation: AdapterOperation) -> AdapterInvocationPlan {
        let supported = self.descriptor().support_level == AdapterSupportLevel::Supported
            && self.supports(operation);
        let phases = phases_for(operation);
        let outputs = outputs_for(operation);
        let failure_modes = if supported {
            vec![
                AdapterFailureKind::ExecutableMissing,
                AdapterFailureKind::LaunchFailed,
                AdapterFailureKind::WindowNotFound,
                AdapterFailureKind::WindowMismatch,
                AdapterFailureKind::ExtractionFailed,
                AdapterFailureKind::OperatorBlocked,
                AdapterFailureKind::TimedOut,
            ]
        } else {
            vec![AdapterFailureKind::NotImplemented]
        };

        AdapterInvocationPlan {
            supported,
            support_level: self.descriptor().support_level,
            typed_only: true,
            requires_operator_presence: matches!(
                operation,
                AdapterOperation::ExecuteWrite | AdapterOperation::VerifyPostAction
            ),
            phases,
            outputs,
            failure_modes,
            notes: self.descriptor().execution_notes.clone(),
        }
    }
}

pub struct AdapterRegistry {
    adapters: Vec<Box<dyn VehicleToolAdapter>>,
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self {
            adapters: vec![
                Box::new(OpComAdapter::new(WindowsVersion::Win7)),
                Box::new(OpComAdapter::new(WindowsVersion::Win11)),
                Box::new(RenolinkAdapter::new(WindowsVersion::Win7)),
                Box::new(RenolinkAdapter::new(WindowsVersion::Win11)),
                Box::new(LexiaAdapter::new(WindowsVersion::Win7)),
                Box::new(LexiaAdapter::new(WindowsVersion::Win11)),
                Box::new(XentryAdapter::new(WindowsVersion::Win7)),
                Box::new(XentryAdapter::new(WindowsVersion::Win11)),
            ],
        }
    }
}

impl AdapterRegistry {
    #[must_use]
    pub fn descriptors(&self) -> Vec<AdapterDescriptor> {
        self.adapters
            .iter()
            .map(|adapter| adapter.descriptor().clone())
            .collect()
    }

    #[must_use]
    pub fn resolve(
        &self,
        tool_family: ToolFamily,
        windows_version: WindowsVersion,
    ) -> Option<&dyn VehicleToolAdapter> {
        self.adapters
            .iter()
            .find(|adapter| {
                let descriptor = adapter.descriptor();
                descriptor.tool_family == tool_family
                    && descriptor.windows_version == windows_version
            })
            .map(Box::as_ref)
    }
}

#[must_use]
pub fn adapter_catalog() -> Vec<AdapterDescriptor> {
    AdapterRegistry::default().descriptors()
}

fn phases_for(operation: AdapterOperation) -> Vec<AdapterPhase> {
    match operation {
        AdapterOperation::ConnectTool => {
            vec![AdapterPhase::Launch, AdapterPhase::AttachInterface]
        }
        AdapterOperation::ExportReport => vec![
            AdapterPhase::Launch,
            AdapterPhase::AttachInterface,
            AdapterPhase::Navigate,
            AdapterPhase::ExportReport,
        ],
        _ => vec![
            AdapterPhase::Launch,
            AdapterPhase::AttachInterface,
            AdapterPhase::Navigate,
            AdapterPhase::Extract,
        ],
    }
}

fn outputs_for(operation: AdapterOperation) -> Vec<ExtractionKind> {
    match operation {
        AdapterOperation::DiscoverVehicle => {
            vec![ExtractionKind::VehicleIdentity, ExtractionKind::ToolHealth]
        }
        AdapterOperation::IdentifyEcu => {
            vec![ExtractionKind::EcuIdentity, ExtractionKind::ToolHealth]
        }
        AdapterOperation::ReadDtcs => vec![ExtractionKind::DtcList, ExtractionKind::ToolHealth],
        AdapterOperation::ReadLiveData => {
            vec![ExtractionKind::LiveData, ExtractionKind::ToolHealth]
        }
        AdapterOperation::ExportReport => {
            vec![ExtractionKind::ReportExport, ExtractionKind::ToolHealth]
        }
        _ => vec![ExtractionKind::ToolHealth],
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AdapterOperation, AdapterRegistry, AdapterSupportLevel, ToolFamily, WindowsVersion,
    };

    #[test]
    fn registry_resolves_each_tool_variant() {
        let registry = AdapterRegistry::default();

        for tool in [
            ToolFamily::OpCom,
            ToolFamily::Renolink,
            ToolFamily::Lexia,
            ToolFamily::Xentry,
        ] {
            assert!(registry.resolve(tool, WindowsVersion::Win7).is_some());
            assert!(registry.resolve(tool, WindowsVersion::Win11).is_some());
        }
    }

    #[test]
    fn opcom_is_supported_while_other_initial_variants_are_planned() {
        let registry = AdapterRegistry::default();
        let opcom = registry
            .resolve(ToolFamily::OpCom, WindowsVersion::Win11)
            .expect("opcom win11 exists");
        let lexia = registry
            .resolve(ToolFamily::Lexia, WindowsVersion::Win11)
            .expect("lexia win11 exists");

        assert_eq!(
            opcom.descriptor().support_level,
            AdapterSupportLevel::Supported
        );
        assert_eq!(
            lexia.descriptor().support_level,
            AdapterSupportLevel::Planned
        );
        assert!(opcom.plan(AdapterOperation::ReadDtcs).supported);
        assert!(!lexia.plan(AdapterOperation::ReadDtcs).supported);
    }
}
