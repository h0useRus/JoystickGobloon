//! A view's picture: a photo of the device (PNG with a transparent background, or JPEG) or an
//! SVG drawing.
//! Hotspots are in the picture's own units: pixels of a photo, the viewBox of a drawing.

use std::path::Path;

use crate::svg::{Palette, themed, view_box};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picture {
    /// PNG bytes and their size in pixels.
    Png {
        bytes: Vec<u8>,
        size: [u32; 2],
    },
    /// JPEG bytes and their size in pixels (no transparency: shot on the page's background).
    Jpeg {
        bytes: Vec<u8>,
        size: [u32; 2],
    },
    Svg(String),
}

impl Picture {
    /// Reads a `.png` or `.svg` file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let name = path.display();
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        match extension.as_str() {
            "png" => {
                let bytes = std::fs::read(path).map_err(|e| format!("{name}: {e}"))?;
                let size = png_size(&bytes).ok_or_else(|| format!("{name}: not a PNG"))?;
                Ok(Self::Png { bytes, size })
            }
            "jpg" | "jpeg" => {
                let bytes = std::fs::read(path).map_err(|e| format!("{name}: {e}"))?;
                let size = jpeg_size(&bytes).ok_or_else(|| format!("{name}: not a JPEG"))?;
                Ok(Self::Jpeg { bytes, size })
            }
            "svg" => std::fs::read_to_string(path)
                .map(Self::Svg)
                .map_err(|e| format!("{name}: {e}")),
            _ => Err(format!("{name}: pictures are .png, .jpg or .svg")),
        }
    }

    /// x, y, width, height of the picture in hotspot units.
    pub fn bounds(&self) -> Option<[f32; 4]> {
        match self {
            Self::Png { size: [w, h], .. } | Self::Jpeg { size: [w, h], .. } => {
                Some([0.0, 0.0, *w as f32, *h as f32])
            }
            Self::Svg(svg) => view_box(svg),
        }
    }
}

impl Picture {
    /// The picture as SVG: a drawing themed, a photo embedded as an image.
    pub fn as_svg(&self, palette: &Palette) -> String {
        let (mime, bytes, [w, h]) = match self {
            Self::Svg(svg) => return themed(svg, palette),
            Self::Png { bytes, size } => ("image/png", bytes, size),
            Self::Jpeg { bytes, size } => ("image/jpeg", bytes, size),
        };
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}"><image width="{w}" height="{h}" href="data:{mime};base64,{}"/></svg>"#,
            base64(bytes)
        )
    }

    /// Pixels to show: a photo at its own size, a drawing `scale` times its viewBox.
    pub fn rasterize(&self, palette: &Palette, scale: f32) -> Result<Rgba, String> {
        let scale = if matches!(self, Self::Svg(_)) {
            scale
        } else {
            1.0
        };
        rasterize_svg(&self.as_svg(palette), scale, None)
    }
}

/// Premultiplied RGBA pixels, row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    pub size: [u32; 2],
    pub pixels: Vec<u8>,
}

impl Rgba {
    /// Windows shows what is behind a window through pixels that are (nearly) black in some
    /// cases (transparent windows, window captures): the darkest parts of a photo would show as
    /// specks. A floor a little above black keeps them dark and opaque.
    pub fn lift_blacks(&mut self) {
        const FLOOR: u16 = 16;
        for p in self.pixels.as_chunks_mut::<4>().0 {
            let floor = (FLOOR * u16::from(p[3]) / 255) as u8;
            for c in &mut p[..3] {
                *c = (*c).max(floor);
            }
        }
    }
}

/// Renders `svg` at `scale` over `background` (transparent without one).
pub fn rasterize_svg(svg: &str, scale: f32, background: Option<[u8; 3]>) -> Result<Rgba, String> {
    use resvg::{tiny_skia, usvg};
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    let size = tree
        .size()
        .to_int_size()
        .scale_by(scale)
        .ok_or("empty picture")?;
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("empty picture")?;
    if let Some([r, g, b]) = background {
        pixmap.fill(tiny_skia::Color::from_rgba8(r, g, b, 255));
    }
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(Rgba {
        size: [size.width(), size.height()],
        pixels: pixmap.take(),
    })
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Width and height from a PNG's IHDR chunk.
pub fn png_size(bytes: &[u8]) -> Option<[u32; 2]> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || !bytes.starts_with(SIGNATURE) || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let number = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    Some([number(16), number(20)])
}

/// Width and height from a JPEG's start-of-frame segment.
pub fn jpeg_size(bytes: &[u8]) -> Option<[u32; 2]> {
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut at = 2;
    while at + 9 < bytes.len() {
        if bytes[at] != 0xFF {
            return None;
        }
        let marker = bytes[at + 1];
        let length = usize::from(u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]));
        // SOF0..SOF15 except DHT (C4), JPG (C8) and DAC (CC).
        if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
            let h = u16::from_be_bytes([bytes[at + 5], bytes[at + 6]]);
            let w = u16::from_be_bytes([bytes[at + 7], bytes[at + 8]]);
            return Some([u32::from(w), u32::from(h)]);
        }
        at += 2 + length;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_png_size() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend_from_slice(&1200u32.to_be_bytes());
        png.extend_from_slice(&900u32.to_be_bytes());
        assert_eq!(png_size(&png), Some([1200, 900]));
        assert_eq!(png_size(b"GIF89a"), None);
    }

    #[test]
    fn rasterizes_drawings_and_photos() {
        let svg = Picture::Svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 20"><rect class="body" width="10" height="20"/></svg>"#.into(),
        );
        let rgba = svg.rasterize(&Palette::dark(), 2.0).unwrap();
        assert_eq!(rgba.size, [20, 40]);
        assert_eq!(rgba.pixels.len(), 20 * 40 * 4);
        assert_eq!(rgba.pixels[3], 255, "drawn opaque");
        assert_eq!(base64(b"Ma"), "TWE=");
    }

    #[test]
    fn reads_jpeg_size() {
        // SOI, an APP0 segment of 4 bytes, SOF0 with height 600 and width 800.
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x02,
            0x58, 0x03, 0x20, 0x03,
        ];
        assert_eq!(jpeg_size(&jpeg), Some([800, 600]));
        assert_eq!(jpeg_size(b"GIF89a"), None);
    }
}
