use spike_e1_rust::presentation::{PresentationExportError, SCHEMA_VERSION, export_e1_s2_cold};

#[tauri::command]
pub fn contract_version() -> &'static str {
    SCHEMA_VERSION
}

#[tauri::command]
pub fn run_e1_s2_cold() -> Result<String, String> {
    serialize_e1_result(export_e1_s2_cold())
}

fn serialize_e1_result(
    result: Result<
        spike_e1_rust::presentation::SimulationPresentationSnapshot,
        PresentationExportError,
    >,
) -> Result<String, String> {
    let snapshot = result.map_err(bridge_error_code)?;
    snapshot
        .to_json()
        .map_err(|_| "presentation_contract_invalid".to_owned())
}

fn bridge_error_code(error: PresentationExportError) -> String {
    match error {
        PresentationExportError::Engine(_) => "engine_execution_failed".to_owned(),
        PresentationExportError::Contract(_) => "presentation_contract_invalid".to_owned(),
        PresentationExportError::UnexpectedOutcome(_) => "unexpected_execution_outcome".to_owned(),
    }
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
