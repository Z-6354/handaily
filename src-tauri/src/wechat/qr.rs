//! 二维码内容 → PNG data URL

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use image::{ExtendedColorType, ImageEncoder, Luma};

pub fn to_qr_data_url(content: &str) -> Result<String, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("二维码内容为空".into());
    }
    if trimmed.starts_with("data:image/") {
        return Ok(trimmed.to_string());
    }
    if looks_like_raw_base64(trimmed) {
        let mime = if trimmed.starts_with("/9j/") {
            "jpeg"
        } else {
            "png"
        };
        return Ok(format!("data:image/{mime};base64,{trimmed}"));
    }
    let code = qrcode::QrCode::new(trimmed.as_bytes()).map_err(|e| e.to_string())?;
    let img = code
        .render::<Luma<u8>>()
        .min_dimensions(320, 320)
        .max_dimensions(320, 320)
        .build();
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(img.as_raw(), img.width(), img.height(), ExtendedColorType::L8)
        .map_err(|e| e.to_string())?;
    Ok(format!("data:image/png;base64,{}", B64.encode(bytes)))
}

fn looks_like_raw_base64(value: &str) -> bool {
    if value.starts_with("http://") || value.starts_with("https://") {
        return false;
    }
    if value.len() < 64 {
        return false;
    }
    value
        .chars()
        .take(256)
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=' || c == '\r' || c == '\n')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_data_url_is_png() {
        let url = to_qr_data_url("weixin://ilink/bot/test").unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
    }
}
