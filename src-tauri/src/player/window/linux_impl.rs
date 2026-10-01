use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::{Mutex, OnceLock};
use x11::xlib;

pub type Surface = u64;

pub fn to_raw(surface: Surface) -> isize {
    surface as isize
}

pub fn from_raw(raw: isize) -> Surface {
    raw as Surface
}

struct Display(*mut xlib::Display);
unsafe impl Send for Display {}

static DISPLAY: OnceLock<Mutex<Display>> = OnceLock::new();

fn with_display<R>(f: impl FnOnce(*mut xlib::Display) -> R) -> Option<R> {
    let conn = DISPLAY.get_or_init(|| {
        Mutex::new(Display(unsafe { xlib::XOpenDisplay(std::ptr::null()) }))
    });
    let guard = conn.lock().ok()?;
    if guard.0.is_null() {
        return None;
    }
    let result = f(guard.0);
    unsafe { xlib::XFlush(guard.0) };
    Some(result)
}

pub fn main_surface(window: &tauri::WebviewWindow) -> Option<Surface> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Xlib(h) => Some(h.window as Surface),
        RawWindowHandle::Xcb(h) => Some(h.window.get() as Surface),
        _ => None,
    }
}

pub fn create_child(parent: Surface) -> Result<Surface, String> {
    with_display(|d| unsafe {
        let screen = xlib::XDefaultScreen(d);
        let black = xlib::XBlackPixel(d, screen);
        xlib::XCreateSimpleWindow(d, parent as xlib::Window, 0, 0, 1, 1, 0, black, black) as Surface
    })
    .filter(|w| *w != 0)
    .ok_or_else(|| "could not create the X11 video window".to_string())
}

pub fn resize(surface: Surface, x: i32, y: i32, width: i32, height: i32) {
    with_display(|d| unsafe {
        xlib::XMoveResizeWindow(d, surface as xlib::Window, x, y, width.max(1) as u32, height.max(1) as u32);
        xlib::XRaiseWindow(d, surface as xlib::Window);
    });
}

pub fn bring_to_front(surface: Surface) {
    with_display(|d| unsafe {
        xlib::XRaiseWindow(d, surface as xlib::Window);
    });
}

pub fn set_visible(surface: Surface, visible: bool) {
    with_display(|d| unsafe {
        if visible {
            xlib::XMapRaised(d, surface as xlib::Window);
        } else {
            xlib::XUnmapWindow(d, surface as xlib::Window);
        }
    });
}

pub fn client_to_screen(main: Surface, x: i32, y: i32) -> (i32, i32) {
    with_display(|d| unsafe {
        let root = xlib::XDefaultRootWindow(d);
        let (mut sx, mut sy, mut child) = (0, 0, 0);
        xlib::XTranslateCoordinates(d, main as xlib::Window, root, x, y, &mut sx, &mut sy, &mut child);
        (sx, sy)
    })
    .unwrap_or((x, y))
}

pub fn capture_colors(surface: Surface, width: u32) -> Option<(u32, u32, Vec<u8>)> {
    with_display(|d| unsafe {
        let window = surface as xlib::Window;
        let mut attrs: xlib::XWindowAttributes = std::mem::zeroed();
        if xlib::XGetWindowAttributes(d, window, &mut attrs) == 0 || attrs.map_state != xlib::IsViewable {
            return None;
        }
        let (src_w, src_h) = (attrs.width, attrs.height);
        if src_w < 16 || src_h < 16 {
            return None;
        }
        let image = xlib::XGetImage(d, window, 0, 0, src_w as u32, src_h as u32, xlib::XAllPlanes(), xlib::ZPixmap);
        if image.is_null() {
            return None;
        }
        let out_w = width.max(8) as i32;
        let out_h = ((out_w as i64 * src_h as i64) / src_w as i64).max(4) as i32;
        let mut pixels = Vec::with_capacity((out_w * out_h * 4) as usize);
        for oy in 0..out_h {
            let sy = (oy * src_h + src_h / 2) / out_h;
            for ox in 0..out_w {
                let sx = (ox * src_w + src_w / 2) / out_w;
                let p = xlib::XGetPixel(image, sx, sy);
                pixels.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, 255]);
            }
        }
        xlib::XDestroyImage(image);
        Some((out_w as u32, out_h as u32, pixels))
    })
    .flatten()
}
