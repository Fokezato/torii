//! Idioma dos textos que saem do Rust (notificações, log de atividade,
//! erros mostrados na tela, menu da bandeja). Espelha a setting
//! "app_language" do front: "pt-BR", "en" ou "auto" (segue o Windows).

/// `tr!("texto em pt {x}", "text in en {x}")` → `String` no idioma atual.
/// Aceita argumentos extras igual `format!`.
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

/// Aplica o valor da setting. Qualquer coisa fora de "pt-BR"/"en" conta
/// como automático.
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
    // SAFETY: sem parâmetros, só lê o idioma da interface do usuário.
    let lang_id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
    lang_id & 0x3ff == LANG_PORTUGUESE
}

#[cfg(not(windows))]
fn system_is_portuguese() -> bool {
    std::env::var("LANG").map(|l| l.starts_with("pt")).unwrap_or(false)
}

/// Itens do menu da bandeja, guardados pra trocar o texto quando o idioma muda.
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
