//! 从 exe 路径提取应用图标（PNG data URL，内存缓存）

use base64::Engine;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::{LazyLock, Mutex};

use image::{ImageBuffer, Rgba};

static ICON_CACHE: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 返回 `data:image/png;base64,...`，失败则 None
pub fn icon_data_url(exe_path: &str) -> Option<String> {
    let path = exe_path.trim();
    if path.is_empty() {
        return None;
    }
    let key = path.to_lowercase();
    if let Ok(cache) = ICON_CACHE.lock() {
        if let Some(cached) = cache.get(&key) {
            return Some(cached.clone());
        }
    }
    let png = extract_icon_png(path)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    let url = format!("data:image/png;base64,{b64}");
    if let Ok(mut cache) = ICON_CACHE.lock() {
        cache.insert(key, url.clone());
    }
    Some(url)
}

/// 从 aggregation_key 推断可用来取图标的 exe 路径
pub fn resolve_icon_path(key: &str, exe_path: &str) -> Option<String> {
    if !exe_path.is_empty() && Path::new(exe_path).exists() {
        return Some(exe_path.to_string());
    }
    let k = key.trim();
    if k.is_empty() {
        return None;
    }
    if Path::new(k).exists() {
        return Some(k.to_string());
    }
    if k.ends_with(".exe") {
        return Some(k.to_string());
    }
    None
}

#[cfg(windows)]
fn extract_icon_png(exe_path: &str) -> Option<Vec<u8>> {
    use std::mem::size_of;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits,
        SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL};

    const SIZE: i32 = 32;

    unsafe {
        let wide: Vec<u16> = OsStr::new(exe_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut shfi = SHFILEINFOW::default();
        let _ = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            Default::default(),
            Some(&mut shfi),
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if shfi.hIcon.is_invalid() {
            return None;
        }

        let hdc_screen = windows::Win32::Graphics::Gdi::GetDC(Some(HWND::default()));
        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        let hbm = CreateCompatibleBitmap(hdc_screen, SIZE, SIZE);
        let old = SelectObject(hdc_mem, hbm.into());
        let _ = DrawIconEx(hdc_mem, 0, 0, shfi.hIcon, SIZE, SIZE, 0, None, DI_NORMAL);
        let _ = SelectObject(hdc_mem, old);

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIZE,
                biHeight: -SIZE,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let stride = (SIZE * 4) as usize;
        let mut pixels = vec![0u8; stride * SIZE as usize];
        let lines = GetDIBits(
            hdc_mem,
            hbm,
            0,
            SIZE as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        let _ = DeleteObject(hbm.into());
        let _ = DeleteDC(hdc_mem);
        let _ = windows::Win32::Graphics::Gdi::ReleaseDC(Some(HWND::default()), hdc_screen);
        let _ = DestroyIcon(shfi.hIcon);

        if lines == 0 {
            return None;
        }

        let mut rgba = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(SIZE as u32, SIZE as u32);
        for y in 0..SIZE as u32 {
            for x in 0..SIZE as u32 {
                let i = (y as usize * stride + x as usize * 4) as usize;
                let b = pixels[i];
                let g = pixels[i + 1];
                let r = pixels[i + 2];
                let a = pixels[i + 3];
                rgba.put_pixel(x, y, Rgba([r, g, b, if a == 0 { 255 } else { a }]));
            }
        }

        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(rgba)
            .write_to(&mut buf, image::ImageFormat::Png)
            .ok()?;
        Some(buf.into_inner())
    }
}

#[cfg(not(windows))]
fn extract_icon_png(_exe_path: &str) -> Option<Vec<u8>> {
    None
}
