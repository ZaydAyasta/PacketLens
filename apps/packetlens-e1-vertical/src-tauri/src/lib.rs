//! Bridge experimental en proceso: Vue recibe un DTO ya proyectado y no conoce
//! los tipos del motor ni ejecuta reglas de ARP/CAM.

use spike_e1_rust::presentation::{export_e1_s2_cold, PresentationExportError, SCHEMA_VERSION};

#[tauri::command]
pub fn contract_version() -> &'static str {
    SCHEMA_VERSION
}

#[tauri::command]
pub fn run_e1_s2_cold() -> Result<String, String> {
    serialize_e1_result(export_e1_s2_cold())
}

fn serialize_e1_result(
    result: Result<spike_e1_rust::presentation::SimulationPresentationSnapshot, PresentationExportError>,
) -> Result<String, String> {
    let snapshot = result.map_err(bridge_error_code)?;
    snapshot.to_json().map_err(|_| "presentation_contract_invalid".to_owned())
}

fn bridge_error_code(error: PresentationExportError) -> String {
    match error {
        PresentationExportError::Engine(_) => "engine_execution_failed".to_owned(),
        PresentationExportError::Contract(_) => "presentation_contract_invalid".to_owned(),
        PresentationExportError::UnexpectedOutcome(_) => "unexpected_execution_outcome".to_owned(),
    }
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![contract_version, run_e1_s2_cold])
        .run(tauri::generate_context!())
        .expect("error while running the PacketLens E1 vertical application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use spike_e1_rust::{EngineError, presentation::PresentationExportError};

    #[test]
    fn maps_engine_execution_failures_without_exposing_engine_types_to_vue() {
        assert_eq!(
            serialize_e1_result(Err(PresentationExportError::Engine(EngineError::Cancelled))),
            Err("engine_execution_failed".to_owned())
        );
    }
}
