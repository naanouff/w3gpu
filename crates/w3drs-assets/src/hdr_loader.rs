use thiserror::Error;

#[derive(Debug, Error)]
pub enum HdrError {
    #[error("image decode error: {0}")]
    Decode(#[from] image::ImageError),
}

/// Equirectangular HDR image with linear RGB float pixels.
pub struct HdrImage {
    pub pixels: Vec<[f32; 3]>,
    pub width: u32,
    pub height: u32,
}

pub fn load_hdr_from_bytes(bytes: &[u8]) -> Result<HdrImage, HdrError> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Hdr)
        .map_err(HdrError::Decode)?;
    let rgb32 = img.into_rgb32f();
    let width = rgb32.width();
    let height = rgb32.height();
    let pixels: Vec<[f32; 3]> = rgb32.pixels().map(|p| [p[0], p[1], p[2]]).collect();
    Ok(HdrImage {
        pixels,
        width,
        height,
    })
}
