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

pub type Surface = HWND;

pub fn to_raw(surface: Surface) -> isize {
    surface.0 as isize
}

pub fn from_raw(raw: isize) -> Surface {
    HWND(raw as *mut std::ffi::c_void)
}

pub fn main_surface(window: &tauri::WebviewWindow) -> Option<Surface> {
    window.hwnd().ok()
}

const CLASS_NAME: windows::core::PCWSTR = w!("ToriiVideoSurface");
static REGISTER_ONCE: Once = Once::new();

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
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

pub fn create_child(parent: HWND) -> Result<HWND, String> {
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
        )
        .map_err(|e| e.to_string())?;
        let _ = SetWindowPos(hwnd, Some(HWND_TOP), 0, 0, 1, 1, SWP_NOACTIVATE);
        Ok(hwnd)
    }
}

pub fn resize(hwnd: HWND, x: i32, y: i32, width: i32, height: i32) {
    unsafe {
        let _ = SetWindowPos(hwnd, Some(HWND_TOP), x, y, width.max(1), height.max(1), SWP_NOACTIVATE);
    }
}

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

pub fn capture_colors(hwnd: HWND, width: u32) -> Option<(u32, u32, Vec<u8>)> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
        SetStretchBltMode, StretchBlt, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HALFTONE, SRCCOPY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsWindowVisible};

    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return None;
        }
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect).ok()?;
        let (src_w, src_h) = (rect.right - rect.left, rect.bottom - rect.top);
        if src_w < 16 || src_h < 16 {
            return None;
        }
        let out_w = width.max(8) as i32;
        let out_h = ((out_w as i64 * src_h as i64) / src_w as i64).max(4) as i32;

        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, out_w, out_h);
        let old = SelectObject(mem, bitmap.into());
        SetStretchBltMode(mem, HALFTONE);
        let copied = StretchBlt(mem, 0, 0, out_w, out_h, Some(screen), rect.left, rect.top, src_w, src_h, SRCCOPY).as_bool();

        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: out_w,
                biHeight: -out_h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (out_w * out_h * 4) as usize];
        let lines = GetDIBits(mem, bitmap, 0, out_h as u32, Some(pixels.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);

        SelectObject(mem, old);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);

        if !copied || lines == 0 {
            return None;
        }
        for px in pixels.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }
        Some((out_w as u32, out_h as u32, pixels))
    }
}
