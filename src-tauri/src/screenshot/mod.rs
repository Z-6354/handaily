//! 前台窗口截图（分析完即弃，不落盘）

#[cfg(windows)]
pub fn capture_foreground_jpeg(max_width: u32, quality: u8) -> Result<Vec<u8>, String> {
    use image::codecs::jpeg::JpegEncoder;
    use image::{DynamicImage, ImageBuffer, Rgba};
    use std::io::Cursor;
    use std::mem::size_of;
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits,
        GetWindowDC, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        SRCCOPY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect};

    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.0.is_null() {
            return Err("无前台窗口".into());
        }

        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect)
            .map_err(|e| format!("GetWindowRect 失败: {e}"))?;

        let width = (rect.right - rect.left).max(1) as u32;
        let height = (rect.bottom - rect.top).max(1) as u32;
        if width < 32 || height < 32 {
            return Err("窗口过小".into());
        }

        let hdc_screen = GetWindowDC(Some(hwnd));
        if hdc_screen.is_invalid() {
            return Err("GetWindowDC 失败".into());
        }

        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        if hdc_mem.is_invalid() {
            let _ = ReleaseDC(Some(hwnd), hdc_screen);
            return Err("CreateCompatibleDC 失败".into());
        }

        let hbm = CreateCompatibleBitmap(hdc_screen, width as i32, height as i32);
        if hbm.is_invalid() {
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(Some(hwnd), hdc_screen);
            return Err("CreateCompatibleBitmap 失败".into());
        }

        let old = SelectObject(hdc_mem, hbm.into());
        let blt_ok = BitBlt(hdc_mem, 0, 0, width as i32, height as i32, Some(hdc_screen), 0, 0, SRCCOPY);
        let _ = SelectObject(hdc_mem, old);
        let _ = ReleaseDC(Some(hwnd), hdc_screen);

        if blt_ok.is_err() {
            let _ = DeleteObject(hbm.into());
            let _ = DeleteDC(hdc_mem);
            return Err("BitBlt 失败".into());
        }

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let stride = (width * 4) as usize;
        let mut pixels = vec![0u8; stride * height as usize];
        let lines = GetDIBits(
            hdc_mem,
            hbm,
            0,
            height,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        if lines == 0 {
            let _ = DeleteObject(hbm.into());
            let _ = DeleteDC(hdc_mem);
            return Err("GetDIBits 失败".into());
        }

        let _ = DeleteObject(hbm.into());
        let _ = DeleteDC(hdc_mem);

        let mut rgba = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let i = (y as usize * stride + x as usize * 4) as usize;
                let b = pixels[i];
                let g = pixels[i + 1];
                let r = pixels[i + 2];
                rgba.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }

        let mut img = DynamicImage::ImageRgba8(rgba);
        if width > max_width {
            let new_h = (height as f32 * max_width as f32 / width as f32).round() as u32;
            img = img.resize(max_width, new_h.max(1), image::imageops::FilterType::Triangle);
        }

        let mut buf = Cursor::new(Vec::new());
        let mut encoder = JpegEncoder::new_with_quality(&mut buf, quality);
        encoder
            .encode_image(&img)
            .map_err(|e| format!("JPEG 编码失败: {e}"))?;
        Ok(buf.into_inner())
    }
}

#[cfg(not(windows))]
pub fn capture_foreground_jpeg(_max_width: u32, _quality: u8) -> Result<Vec<u8>, String> {
    Err("截图仅支持 Windows".into())
}
