//! Child HWND nativa pro vídeo do libvlc renderizar dentro — não dá pra
//! desenhar vídeo direto no WebView2 (é um controle Chromium, não aceita
//! HWND estrangeira "dentro" do DOM). Em vez disso: cria uma janela filha
//! de verdade, do lado do WebView2 principal (mesmo parent HWND), passa o
//! handle pro libvlc (`libvlc_media_player_set_hwnd`) e reposiciona ela por
//! cima da área onde o `<video-slot>` do React fica, a cada resize/scroll
//! (ver `commands/player.rs::player_resize`). Overlay de controles (barra
//! de progresso etc.) é uma SEGUNDA janela Tauri transparente por cima
//! dessa — mesmo truque já usado pra `NotificationWindow` nesse projeto.

use std::sync::Once;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{ClientToScreen, GetStockObject, BLACK_BRUSH, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetWindow, RegisterClassExW, SetWindowPos, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, GW_HWNDNEXT, GW_HWNDPREV, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE,
    WNDCLASSEXW, WS_CHILD, WS_EX_NOACTIVATE,
};

const CLASS_NAME: windows::core::PCWSTR = w!("ToriiVideoSurface");
static REGISTER_ONCE: Once = Once::new();

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // O vídeo em si é pintado pelo vout do libvlc direto na superfície
    // dessa HWND (D3D11/D3D9), não via WM_PAINT — não precisa de lógica de
    // desenho aqui, só repassar pro handler padrão.
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn register_class() {
    REGISTER_ONCE.call_once(|| unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        RegisterClassExW(&class);
    });
}

/// Cria a HWND filha, invisível/0x0 até o primeiro `resize` real vindo do
/// React (evita um quadrado preto de 1x1 piscando no canto antes do layout
/// assentar).
pub fn create_child(parent: HWND) -> windows::core::Result<HWND> {
    register_class();
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let hwnd = CreateWindowExW(
            WS_EX_NOACTIVATE,
            CLASS_NAME,
            w!(""),
            WS_CHILD,
            0,
            0,
            1,
            1,
            Some(parent),
            None,
            Some(hinstance.into()),
            None,
        )?;
        // O WebView2 é ele mesmo uma HWND filha da mesma janela principal —
        // sem forçar HWND_TOP aqui, a ordem de criação/repaint do WebView2
        // podia deixar nossa janela atrás dele (vídeo tocando com som mas
        // tela preta, já visto ao vivo: WebView2 cobrindo por cima).
        let _ = SetWindowPos(hwnd, Some(HWND_TOP), 0, 0, 1, 1, SWP_NOACTIVATE);
        Ok(hwnd)
    }
}

pub fn resize(hwnd: HWND, x: i32, y: i32, width: i32, height: i32) {
    unsafe {
        // HWND_TOP de novo (não só a primeira vez): o WebView2 pode
        // reafirmar o próprio z-order em repaints/navegação, empurrando
        // nossa janela pra trás de novo.
        let _ = SetWindowPos(hwnd, Some(HWND_TOP), x, y, width.max(1), height.max(1), SWP_NOACTIVATE);
    }
}

/// Reafirma HWND_TOP sem mexer em posição/tamanho. O WebView2 reafirma o
/// PRÓPRIO z-order em momentos imprevisíveis (ex. quando a janela de
/// overlay termina de inicializar o WebView2 dela) — 1 SetWindowPos na
/// criação/resize não é suficiente pra sempre, isso aqui é chamado de novo
/// periodicamente (ver `PlayerOverlay` no front) pra brigar de volta.
pub fn bring_to_front(hwnd: HWND) {
    unsafe {
        let _ = SetWindowPos(hwnd, Some(HWND_TOP), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}

pub fn set_visible(hwnd: HWND, visible: bool) {
    unsafe {
        let _ = ShowWindow(hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
    }
}

/// Converte `(x, y)` relativo à área de conteúdo de `hwnd` pra coordenada
/// ABSOLUTA de tela via Win32 puro — usado pra posicionar a janela de
/// overlay exatamente onde a HWND filha do vídeo (mesma referência: área de
/// conteúdo da janela principal) está de verdade. Tanto `innerPosition()` do
/// front (JS/WRY) quanto `Window::inner_position()` do Tauri no lado Rust se
/// mostraram, ao vivo, alguns pixels errados quando a janela principal está
/// MAXIMIZADA num monitor QuadHD/ultrawide (bug real reportado 2x — os 2
/// fixes anteriores tentaram compensar por cima da posição que o Tauri
/// calcula, que aparentemente tem o mesmo desvio nos dois lados já que
/// ambos passam pelo mesmo código do WRY por baixo). `ClientToScreen` evita
/// o Tauri inteiro pra essa conta, direto do Win32.
pub fn client_to_screen(hwnd: HWND, x: i32, y: i32) -> (i32, i32) {
    let mut point = POINT { x, y };
    unsafe {
        let _ = ClientToScreen(hwnd, &mut point);
    }
    (point.x, point.y)
}

fn next_window(hwnd: HWND, dir: windows::Win32::UI::WindowsAndMessaging::GET_WINDOW_CMD) -> Option<HWND> {
    unsafe { GetWindow(hwnd, dir).ok().filter(|h| !h.is_invalid()) }
}

/// Garante a overlay LOGO ACIMA da janela principal no z-order global — sem
/// usar topmost (que cobria diálogo de arquivo e outros apps, bug real
/// reportado). Se já está acima, não mexe; senão encaixa ela entre a
/// principal e a janela que estava imediatamente acima dela, então nunca
/// sobe por cima de diálogo/app que o usuário trouxe pra frente.
pub fn ensure_above(overlay: HWND, main: HWND) {
    let mut h = next_window(overlay, GW_HWNDNEXT);
    let mut steps = 0;
    while let Some(cur) = h {
        if cur == main {
            return;
        }
        steps += 1;
        if steps > 5000 {
            break;
        }
        h = next_window(cur, GW_HWNDNEXT);
    }
    let insert_after = match next_window(main, GW_HWNDPREV) {
        Some(prev) if prev != overlay => prev,
        _ => HWND_TOP,
    };
    unsafe {
        let _ = SetWindowPos(overlay, Some(insert_after), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}
