//! Downscale and encode (tech doc 13.5): vision models gain nothing above
//! about 1568 px on the long side, and smaller images upload and answer faster.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};
use sidekick_core::{SidekickError, SidekickResult};

/// Longest side sent to vision models.
pub const MAX_LONG_SIDE: u32 = 1568;
/// JPEG quality: text stays readable at about 200 KB per screenshot.
const JPEG_QUALITY: u8 = 85;

/// An encoded image ready to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoded {
    pub bytes: Vec<u8>,
    pub mime_type: &'static str,
    pub width: u32,
    pub height: u32,
}

/// Target size with the long side at most `max`, keeping the aspect ratio.
#[must_use]
pub fn fit(width: u32, height: u32, max: u32) -> (u32, u32) {
    let long = width.max(height);
    if long <= max || long == 0 {
        return (width, height);
    }
    let scale = f64::from(max) / f64::from(long);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "scaled down from u32, so it fits"
    )]
    let s = |v: u32| ((f64::from(v) * scale).round() as u32).max(1);
    if width >= height {
        (max, s(height))
    } else {
        (s(width), max)
    }
}

/// Shrinks the image so the long side is at most `max` (never enlarges).
#[must_use]
pub fn downscale(img: RgbaImage, max: u32) -> RgbaImage {
    let (w, h) = fit(img.width(), img.height(), max);
    if (w, h) == img.dimensions() {
        return img;
    }
    image::imageops::resize(&img, w, h, FilterType::Triangle)
}

/// Encodes as JPEG (no alpha: screens are opaque).
pub fn encode(img: &RgbaImage) -> SidekickResult<Encoded> {
    let rgb = DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(Cursor::new(&mut bytes), JPEG_QUALITY)
        .encode_image(&rgb)
        .map_err(|e| {
            tracing::error!(target: "capture", error = %e, "encode failed");
            SidekickError::internal("Couldn't encode the screenshot.")
        })?;
    Ok(Encoded {
        bytes,
        mime_type: "image/jpeg",
        width: img.width(),
        height: img.height(),
    })
}

/// Downscale, then encode.
pub fn prepare(img: RgbaImage) -> SidekickResult<Encoded> {
    encode(&downscale(img, MAX_LONG_SIDE))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn fit_keeps_aspect_ratio_and_limit() {
        assert_eq!(fit(3840, 2160, 1568), (1568, 882));
        assert_eq!(fit(2160, 3840, 1568), (882, 1568));
        assert_eq!(fit(1568, 1000, 1568), (1568, 1000));
        assert_eq!(fit(800, 600, 1568), (800, 600), "never enlarges");
        assert_eq!(fit(5000, 2, 1568), (1568, 1), "never zero");
        assert_eq!(fit(0, 0, 1568), (0, 0));
    }

    #[test]
    fn downscale_and_encode_a_retina_screen() {
        let img = RgbaImage::from_pixel(2880, 1800, image::Rgba([30, 60, 90, 255]));
        let e = prepare(img).unwrap();
        assert_eq!((e.width, e.height), (1568, 980));
        assert_eq!(e.mime_type, "image/jpeg");
        assert_eq!(&e.bytes[..2], &[0xFF, 0xD8], "JPEG magic");
        let back = image::load_from_memory(&e.bytes).unwrap();
        assert_eq!((back.width(), back.height()), (1568, 980));
    }
}
