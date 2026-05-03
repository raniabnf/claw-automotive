use super::{
    AdapterCapability, AdapterDescriptor, AdapterSupportLevel, ExtractionKind, ExtractionTarget,
    NavigationCheckpoint, ToolFamily, ToolLaunchSpec, VehicleToolAdapter, WindowIdentity,
    WindowsVersion,
};
use crate::adapters::common::AdapterPhase;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpComAdapter {
    descriptor: AdapterDescriptor,
    launch_spec: ToolLaunchSpec,
    navigation_checkpoints: Vec<NavigationCheckpoint>,
    extraction_targets: Vec<ExtractionTarget>,
}

impl OpComAdapter {
    #[must_use]
    pub fn new(windows_version: WindowsVersion) -> Self {
        let suffix = match windows_version {
            WindowsVersion::Win7 => "win7",
            WindowsVersion::Win11 => "win11",
        };
        let windows_note = match windows_version {
            WindowsVersion::Win7 => {
                "Win7 variant keeps legacy process timing and compatibility assumptions"
            }
            WindowsVersion::Win11 => {
                "Win11 variant reserves room for newer automation backend differences"
            }
        };

        Self {
            descriptor: AdapterDescriptor {
                adapter_id: format!("opcom-{suffix}"),
                tool_family: ToolFamily::OpCom,
                windows_version,
                support_level: AdapterSupportLevel::Supported,
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
                    "typed adapter boundary only".to_string(),
                    "desktop session automation only".to_string(),
                    windows_note.to_string(),
                ],
            },
            launch_spec: ToolLaunchSpec {
                executable_name: "op-com.exe".to_string(),
                cli_args: Vec::new(),
                requires_user_session: true,
                expected_process_names: vec!["op-com.exe".to_string(), "opcom.exe".to_string()],
                working_directory_hint: Some("C:\\Program Files\\OP-COM".to_string()),
            },
            navigation_checkpoints: vec![
                NavigationCheckpoint {
                    checkpoint_id: "main_window_ready".to_string(),
                    summary: "main OP-COM shell is visible and idle".to_string(),
                    phase: AdapterPhase::Launch,
                    expected_window: WindowIdentity {
                        title_hint: "OP-COM".to_string(),
                        class_name: None,
                        automation_hint: Some("main_window".to_string()),
                    },
                    blocking_dialogs: vec![
                        "license".to_string(),
                        "interface not connected".to_string(),
                    ],
                },
                NavigationCheckpoint {
                    checkpoint_id: "vehicle_selection".to_string(),
                    summary: "vehicle model selection workflow is reachable".to_string(),
                    phase: AdapterPhase::Navigate,
                    expected_window: WindowIdentity {
                        title_hint: "Vehicle Selection".to_string(),
                        class_name: None,
                        automation_hint: Some("vehicle_tree".to_string()),
                    },
                    blocking_dialogs: vec!["unknown vehicle".to_string()],
                },
                NavigationCheckpoint {
                    checkpoint_id: "fault_code_screen".to_string(),
                    summary: "fault code screen is visible for extraction".to_string(),
                    phase: AdapterPhase::Extract,
                    expected_window: WindowIdentity {
                        title_hint: "Fault Codes".to_string(),
                        class_name: None,
                        automation_hint: Some("fault_codes_grid".to_string()),
                    },
                    blocking_dialogs: vec!["communication error".to_string()],
                },
            ],
            extraction_targets: vec![
                ExtractionTarget {
                    target_id: "vehicle_identity".to_string(),
                    kind: ExtractionKind::VehicleIdentity,
                    source_hint: "vehicle selection header and VIN surface".to_string(),
                    output_key: "vehicle".to_string(),
                },
                ExtractionTarget {
                    target_id: "ecu_identity".to_string(),
                    kind: ExtractionKind::EcuIdentity,
                    source_hint: "active control module banner".to_string(),
                    output_key: "ecu".to_string(),
                },
                ExtractionTarget {
                    target_id: "dtc_grid".to_string(),
                    kind: ExtractionKind::DtcList,
                    source_hint: "fault code grid or export dialog".to_string(),
                    output_key: "dtcs".to_string(),
                },
                ExtractionTarget {
                    target_id: "report_export".to_string(),
                    kind: ExtractionKind::ReportExport,
                    source_hint: "save/export workflow".to_string(),
                    output_key: "report".to_string(),
                },
            ],
        }
    }
}

impl VehicleToolAdapter for OpComAdapter {
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

#[cfg(test)]
mod tests {
    use super::OpComAdapter;
    use crate::adapters::{AdapterOperation, VehicleToolAdapter, WindowsVersion};

    #[test]
    fn opcom_scaffold_exposes_launch_and_extraction_contract() {
        let adapter = OpComAdapter::new(WindowsVersion::Win11);

        assert_eq!(adapter.launch_spec().executable_name, "op-com.exe");
        assert!(adapter.supports(AdapterOperation::ReadDtcs));
        assert!(adapter
            .navigation_checkpoints()
            .iter()
            .any(|checkpoint| checkpoint.checkpoint_id == "fault_code_screen"));
        assert!(adapter
            .extraction_targets()
            .iter()
            .any(|target| target.output_key == "dtcs"));
    }
}
