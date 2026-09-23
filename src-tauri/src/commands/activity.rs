use crate::{activity::LogEntry, state::AppState};
use tauri::State;

#[tauri::command]
pub fn get_activity_log(state: State<'_, AppState>) -> Vec<LogEntry> {
    state.activity.list()
}
