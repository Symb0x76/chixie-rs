//! Previews for fonts that are not downloaded. Each listing carries `font_preview`: a public PNG
//! of the font's name written in the font (black on white, ~6600×2250, ~400 KB). It is fetched
//! once, cropped to the ink, shrunk to [`MASK_H`] px and kept as a small ink mask in
//! `<fonts>/previews/<id>.png`. The mask is tinted with the theme's text colour, so it looks
//! like the vector previews of downloaded fonts and follows dark mode.

use std::path::Path;
use std::sync::Arc;

use gpui_kit::*;
use image::imageops::FilterType;
use image::{Frame, GrayImage, ImageFormat, RgbaImage};

/// Mask height in px (about 2× the largest on-screen preview, for HiDPI).
pub const MASK_H: u32 = 120;
/// Ink below this (0–255) counts as paper when finding the crop box.
const INK_MIN: u8 = 40;

/// Ink coverage per pixel (0 = paper, 255 = full ink).
pub struct InkMask(GrayImage);

impl InkMask {
    /// Crop the server's preview to its ink (plus a small margin) and shrink it.
    pub fn from_preview_png(bytes: &[u8]) -> Result<Self, String> {
        let src = image::load_from_memory_with_format(bytes, ImageFormat::Png).map_err(|e| e.to_string())?.to_luma_alpha8();
        let (w, h) = src.dimensions();
        let mut ink = GrayImage::new(w, h);
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        for (x, y, p) in src.enumerate_pixels() {
            let v = ((255 - p[0] as u32) * p[1] as u32 / 255) as u8;
            ink.put_pixel(x, y, image::Luma([v]));
            if v >= INK_MIN {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
        if x0 > x1 {
            return Err("预览图是空白的".into());
        }
        let pad = ((y1 - y0) / 25).max(2);
        let (x0, y0) = (x0.saturating_sub(pad), y0.saturating_sub(pad));
        let (x1, y1) = ((x1 + pad).min(w - 1), (y1 + pad).min(h - 1));
        let crop = image::imageops::crop_imm(&ink, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image();
        let out_w = ((crop.width() as f32 * MASK_H as f32 / crop.height() as f32).round() as u32).max(1);
        Ok(Self(image::imageops::resize(&crop, out_w, MASK_H, FilterType::Triangle)))
    }

    fn from_cache_png(bytes: &[u8]) -> Option<Self> {
        image::load_from_memory_with_format(bytes, ImageFormat::Png).ok().map(|i| Self(i.to_luma8()))
    }

    fn to_png(&self) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        let _ = self.0.write_to(&mut out, ImageFormat::Png);
        out.into_inner()
    }

    /// Width / height.
    pub fn aspect(&self) -> f32 {
        self.0.width() as f32 / self.0.height() as f32
    }

    /// The mask as a gpui image (BGRA, straight alpha) in one ink colour.
    fn tinted(&self, color: Hsla) -> Arc<RenderImage> {
        let c = color.to_rgb();
        let (r, g, b) = ((c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8);
        let buf: Vec<u8> = self.0.as_raw().iter().flat_map(|&a| [b, g, r, a]).collect();
        let rgba = RgbaImage::from_raw(self.0.width(), self.0.height(), buf).expect("buffer matches size");
        Arc::new(RenderImage::new(vec![Frame::new(rgba)]))
    }
}

/// A ready preview in both ink colours (light and dark theme).
pub struct CloudImages {
    pub light: Arc<RenderImage>,
    pub dark: Arc<RenderImage>,
    pub aspect: f32,
}

impl CloudImages {
    pub fn new(mask: &InkMask) -> Self {
        Self { light: mask.tinted(hsla(0., 0., 0.09, 1.)), dark: mask.tinted(hsla(0., 0., 0.93, 1.)), aspect: mask.aspect() }
    }

    pub fn for_theme(&self, cx: &App) -> Arc<RenderImage> {
        use gpui_kit::component::ActiveTheme as _;
        if cx.theme().mode.is_dark() { self.dark.clone() } else { self.light.clone() }
    }
}

pub enum CloudPreview {
    Loading,
    Ready(Arc<CloudImages>),
    /// No preview URL, download failed or the image was blank.
    Missing,
}

/// Blocking: the cached mask, else download + convert + cache. Run off the UI thread.
pub fn load_or_fetch(fonts_dir: &Path, id: i64, url: &str) -> Result<InkMask, String> {
    let path = fonts_dir.join("previews").join(format!("{id}.png"));
    if let Some(m) = std::fs::read(&path).ok().and_then(|b| InkMask::from_cache_png(&b)) {
        return Ok(m);
    }
    let bytes = sast_writer_core::api::fetch_url(url).map_err(|e| e.to_string())?;
    let mask = InkMask::from_preview_png(&bytes)?;
    if let Some(dir) = path.parent()
        && std::fs::create_dir_all(dir).is_ok()
        && let Err(e) = std::fs::write(&path, mask.to_png())
    {
        log::warn!("caching preview of font {id}: {e}");
    }
    Ok(mask)
}

#[cfg(test)]
mod tests {
    // Not `super::*`: `gpui_kit::*` brings gpui's own `test` macro, which shadows `#[test]`.
    use super::{ImageFormat, InkMask, MASK_H};

    /// A white sheet with one black bar in the middle and transparent margins.
    fn sheet() -> Vec<u8> {
        let mut img = image::RgbaImage::from_pixel(600, 200, image::Rgba([255, 255, 255, 255]));
        for x in 0..600 {
            for y in 0..10 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 0])); // transparent = paper
            }
        }
        for x in 100..500 {
            for y in 80..120 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn crops_to_ink_and_scales_to_mask_height() {
        let m = InkMask::from_preview_png(&sheet()).unwrap();
        assert_eq!(m.0.height(), MASK_H);
        // 400×40 ink + 2 px margin each side → about 10:1.
        assert!((m.aspect() - 404.0 / 44.0).abs() < 0.3, "aspect {}", m.aspect());
        assert!(m.0.get_pixel(m.0.width() / 2, MASK_H / 2)[0] > 200, "middle is ink");
        let back = InkMask::from_cache_png(&m.to_png()).unwrap();
        assert_eq!(back.0.dimensions(), m.0.dimensions());
    }

    #[test]
    fn blank_sheet_is_an_error() {
        let img = image::RgbaImage::from_pixel(50, 50, image::Rgba([255, 255, 255, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        assert!(InkMask::from_preview_png(&out.into_inner()).is_err());
    }
}
