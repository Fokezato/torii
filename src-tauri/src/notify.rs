use crate::{db, state::AppState};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
struct NotifyPayload {
    title: String,
    body: String,
    variant: &'static str,
    image: Option<String>,
    sound: bool,
}

pub async fn notify(
    app: &AppHandle,
    state: &AppState,
    kind: &str,
    title: impl Into<String>,
    body: impl Into<String>,
    variant: &'static str,
    image: Option<String>,
) {
    let settings = match db::settings::get_all(&state.db).await {
        Ok(s) => s,
        Err(_) => return,
    };
    if settings.get("notify_master").map(String::as_str) != Some("1") {
        return;
    }
    if settings.get(kind).map(String::as_str) != Some("1") {
        return;
    }
    let sound = settings.get("notify_sound").map(String::as_str) == Some("1");

    let payload = NotifyPayload {
        title: title.into(),
        body: body.into(),
        variant,
        image,
        sound,
    };
    let _ = app.emit("notify:show", payload);
}
