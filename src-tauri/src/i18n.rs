#[macro_export]
macro_rules! tr {
    ($pt:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        if $crate::i18n::is_english() {
            format!($en $(, $arg)*)
        } else {
            format!($pt $(, $arg)*)
        }
    };
}

use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn is_english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

pub fn apply_setting(value: &str) {
    let english = match value {
        "pt-BR" => false,
        "en" => true,
        _ => !system_is_portuguese(),
    };
    ENGLISH.store(english, Ordering::Relaxed);
}

#[cfg(windows)]
fn system_is_portuguese() -> bool {
    const LANG_PORTUGUESE: u16 = 0x16;
    let lang_id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
    lang_id & 0x3ff == LANG_PORTUGUESE
}

#[cfg(not(windows))]
fn system_is_portuguese() -> bool {
    std::env::var("LANG").map(|l| l.starts_with("pt")).unwrap_or(false)
}

pub struct TrayMenu {
    pub open: tauri::menu::MenuItem<tauri::Wry>,
    pub quit: tauri::menu::MenuItem<tauri::Wry>,
}

impl TrayMenu {
    pub fn refresh(&self) {
        let _ = self.open.set_text(tr!("Abrir", "Open"));
        let _ = self.quit.set_text(tr!("Sair", "Quit"));
    }
}
