use super::{
    AdapterCapability, AdapterDescriptor, AdapterSupportLevel, ExtractionKind, ExtractionTarget,
    NavigationCheckpoint, ToolFamily, ToolLaunchSpec, VehicleToolAdapter, WindowIdentity,
    WindowsVersion,
};
use crate::adapters::common::AdapterPhase;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XentryAdapter {
    descriptor: AdapterDescriptor,
    launch_spec: ToolLaunchSpec,
    navigation_checkpoints: Vec<NavigationCheckpoint>,
    extraction_targets: Vec<ExtractionTarget>,
}

impl XentryAdapter {
    #[must_use]
    pub fn new(windows_version: WindowsVersion) -> Self {
        let suffix = match windows_version {
            WindowsVersion::Win7 => "win7",
            WindowsVersion::Win11 => "win11",
        };

        Self {
            descriptor: AdapterDescriptor {
                adapter_id: format!("xentry-{suffix}"),
                tool_family: ToolFamily::Xentry,
                windows_version,
                support_level: AdapterSupportLevel::Planned,
                user_session_required: true,
                local_only: true,
                capabilities: vec![
                    AdapterCapability::ConnectTool,
                    AdapterCapability::DiscoverVehicle,
                    AdapterCapability::IdentifyEcu,
                    AdapterCapability::ReadDtcs,
                    AdapterCapability::ReadLiveData,
                    AdapterCapability::LoadProcedure,
                    AdapterCapability::SimulateAction,
                    AdapterCapability::ExecuteWrite,
                    AdapterCapability::VerifyPostAction,
                    AdapterCapability::ExportReport,
                ],
                execution_notes: vec![
                    "placeholder adapter under the same contract as op-com".to_string(),
                    "implementation awaits software discovery details".to_string(),
                ],
            },
            launch_spec: ToolLaunchSpec {
                executable_name: "xentry.exe".to_string(),
                cli_args: Vec::new(),
                requires_user_session: true,
                expected_process_names: vec!["xentry.exe".to_string()],
                working_directory_hint: None,
            },
            navigation_checkpoints: vec![NavigationCheckpoint {
                checkpoint_id: "planned_main_window".to_string(),
                summary: "placeholder main window contract".to_string(),
                phase: AdapterPhase::Launch,
                expected_window: WindowIdentity {
                    title_hint: "XENTRY".to_string(),
                    class_name: None,
                    automation_hint: Some("planned".to_string()),
                },
                blocking_dialogs: vec!["planned".to_string()],
            }],
            extraction_targets: vec![ExtractionTarget {
                target_id: "planned_dtcs".to_string(),
                kind: ExtractionKind::DtcList,
                source_hint: "planned extraction surface".to_string(),
                output_key: "dtcs".to_string(),
            }],
        }
    }
}

impl VehicleToolAdapter for XentryAdapter {
    fn descriptor(&self) -> &AdapterDescriptor {
        &self.descriptor
    }

    fn launch_spec(&self) -> &ToolLaunchSpec {
        &self.launch_spec
    }

    fn navigation_checkpoints(&self) -> &[NavigationCheckpoint] {
        &self.navigation_checkpoints
    }

    fn extraction_targets(&self) -> &[ExtractionTarget] {
        &self.extraction_targets
    }
}
