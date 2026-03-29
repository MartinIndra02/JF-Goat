use tauri::State;

use crate::diagnostics::{self, DiagnosticsExportResult};
use crate::error::JfgoatError;
use crate::state::AppState;

#[tauri::command]
pub fn export_diagnostics(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<DiagnosticsExportResult, JfgoatError> {
    diagnostics::export_diagnostics(&app, &state)
        .map_err(JfgoatError::Internal)
}
