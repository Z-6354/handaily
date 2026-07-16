//! Windows 输入/截图（仅供 debug test-api / UI 自动化测试）。

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{ImageBuffer, RgbaImage};
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct CursorPos {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Serialize)]
pub struct ScreenshotPayload {
    pub width: u32,
    pub height: u32,
    pub png_base64: String,
    pub region: ScreenRegion,
}

#[derive(Clone, Serialize)]
pub struct ScreenRegion {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn cursor_position() -> Result<CursorPos, String> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        unsafe {
            let mut pt = POINT { x: 0, y: 0 };
            GetCursorPos(&mut pt).map_err(|e| e.to_string())?;
            Ok(CursorPos { x: pt.x, y: pt.y })
        }
    }
    #[cfg(not(windows))]
    {
        Err("win32 input only supported on Windows".into())
    }
}

pub fn set_cursor_position(x: i32, y: i32) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;
        unsafe {
            SetCursorPos(x, y).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (x, y);
        Err("win32 input only supported on Windows".into())
    }
}

pub fn mouse_button_action(button: &str, action: &str, x: i32, y: i32) -> Result<(), String> {
    set_cursor_position(x, y)?;
    std::thread::sleep(std::time::Duration::from_millis(12));
    let right = button.eq_ignore_ascii_case("right");
    match action {
        "down" => send_mouse_click(right, true, false),
        "up" => send_mouse_click(right, false, true),
        "click" | "" => send_mouse_click(right, true, true),
        other => Err(format!("unknown mouse action: {other}")),
    }
}

#[cfg(windows)]
fn send_mouse_click(right: bool, down: bool, up: bool) -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_RIGHTDOWN,
        MOUSEEVENTF_RIGHTUP,
    };
    if down {
        let input = mouse_input(if right {
            MOUSEEVENTF_RIGHTDOWN
        } else {
            MOUSEEVENTF_LEFTDOWN
        });
        unsafe {
            if SendInput(&[input], std::mem::size_of::<INPUT>() as i32) == 0 {
                return Err("SendInput down failed".into());
            }
        }
    }
    if up {
        if down {
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        let input = mouse_input(if right {
            MOUSEEVENTF_RIGHTUP
        } else {
            MOUSEEVENTF_LEFTUP
        });
        unsafe {
            if SendInput(&[input], std::mem::size_of::<INPUT>() as i32) == 0 {
                return Err("SendInput up failed".into());
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn mouse_input(
    flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
) -> windows::Win32::UI::Input::KeyboardAndMouse::INPUT {
    use windows::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_MOUSE, MOUSEINPUT};
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

#[cfg(not(windows))]
fn send_mouse_click(_right: bool, _down: bool, _up: bool) -> Result<(), String> {
    Err("win32 input only supported on Windows".into())
}

pub fn capture_region_png(x: i32, y: i32, width: i32, height: i32) -> Result<ScreenshotPayload, String> {
    if width <= 0 || height <= 0 {
        return Err("invalid capture size".into());
    }
    let (w, h) = (width as u32, height as u32);
    let img = capture_bgra(x, y, w, h)?;
    let png = encode_png_rgba(&img, w, h)?;
    Ok(ScreenshotPayload {
        width: w,
        height: h,
        png_base64: STANDARD.encode(&png),
        region: ScreenRegion {
            x,
            y,
            width,
            height,
        },
    })
}

pub fn capture_primary_screen_png(max_width: u32) -> Result<ScreenshotPayload, String> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        unsafe {
            let sw = GetSystemMetrics(SM_CXSCREEN);
            let sh = GetSystemMetrics(SM_CYSCREEN);
            if sw <= 0 || sh <= 0 {
                return Err("invalid screen size".into());
            }
            let mut payload = capture_region_png(0, 0, sw, sh)?;
            if payload.width > max_width {
                let bytes = STANDARD
                    .decode(&payload.png_base64)
                    .map_err(|e| e.to_string())?;
                let decoded = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
                let rgba = decoded.to_rgba8();
                let nh = ((payload.height as f64) * (max_width as f64 / payload.width as f64))
                    .round()
                    .max(1.0) as u32;
                let resized = image::imageops::resize(
                    &rgba,
                    max_width,
                    nh,
                    image::imageops::FilterType::Triangle,
                );
                let mut out = Vec::new();
                let mut cursor = std::io::Cursor::new(&mut out);
                resized
                    .write_to(&mut cursor, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
                payload.width = max_width;
                payload.height = nh;
                payload.png_base64 = STANDARD.encode(&out);
                payload.region.width = max_width as i32;
                payload.region.height = nh as i32;
            }
            Ok(payload)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = max_width;
        Err("screenshot only supported on Windows".into())
    }
}

fn encode_png_rgba(bgra: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
    let expected = (w as usize)
        .checked_mul(h as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| "image too large".to_string())?;
    if bgra.len() < expected {
        return Err("short pixel buffer".into());
    }
    let mut rgba: RgbaImage = ImageBuffer::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let i = ((y as usize * w as usize + x as usize) * 4) as usize;
            rgba.put_pixel(x, y, image::Rgba([bgra[i + 2], bgra[i + 1], bgra[i], 255]));
        }
    }
    let mut out = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut out);
    rgba.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(windows)]
fn capture_bgra(x: i32, y: i32, w: u32, h: u32) -> Result<Vec<u8>, String> {
    use std::mem::size_of;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        SRCCOPY,
    };
    unsafe {
        let hdc_screen = GetDC(None);
        if hdc_screen.0.is_null() {
            return Err("GetDC failed".into());
        }
        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        if hdc_mem.0.is_null() {
            ReleaseDC(None, hdc_screen);
            return Err("CreateCompatibleDC failed".into());
        }
        let hbm = CreateCompatibleBitmap(hdc_screen, w as i32, h as i32);
        if hbm.0.is_null() {
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);
            return Err("CreateCompatibleBitmap failed".into());
        }
        let old = SelectObject(hdc_mem, hbm.into());
        let blt = BitBlt(hdc_mem, 0, 0, w as i32, h as i32, Some(hdc_screen), x, y, SRCCOPY);
        if blt.is_err() {
            let _ = SelectObject(hdc_mem, old);
            let _ = DeleteObject(hbm.into());
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);
            return Err("BitBlt failed".into());
        }

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0 as u32,
                ..Default::default()
            },
            ..Default::default()
        };
        let byte_len = (w as usize)
            .checked_mul(h as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| "image too large".to_string())?;
        let mut pixels = vec![0u8; byte_len];
        let lines = GetDIBits(
            hdc_mem,
            hbm,
            0,
            h,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        let _ = SelectObject(hdc_mem, old);
        let _ = DeleteObject(hbm.into());
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(None, hdc_screen);
        if lines == 0 {
            return Err("GetDIBits failed".into());
        }
        Ok(pixels)
    }
}

#[cfg(not(windows))]
fn capture_bgra(_x: i32, _y: i32, _w: u32, _h: u32) -> Result<Vec<u8>, String> {
    Err("screenshot only supported on Windows".into())
}
