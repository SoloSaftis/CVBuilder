//! Portrait: crop to the CV's 25:32 box and make light fixes (brightness, contrast, straighten).
//! No face or body retouching. The original file is never changed; the CV gets a small JPEG copy
//! without any of the original's metadata (such as GPS location).
use base64::Engine as _;
use image::{DynamicImage, ImageBuffer, Rgb, RgbImage};

pub const RATIO: f32 = 25.0 / 32.0;
const OUT_H: u32 = 512;

#[derive(Clone)]
pub struct Edit {
    pub original: RgbImage,
    /// 1.0 = the largest crop that fits; up to 3.0 zooms in.
    pub zoom: f32,
    /// -1.0 … 1.0: where the crop sits within the free space.
    pub x: f32,
    pub y: f32,
    pub brightness: f32,
    pub contrast: f32,
    /// Degrees, -10 … 10.
    pub straighten: f32,
}

impl Edit {
    pub fn open(path: &std::path::Path) -> Result<Edit, String> {
        let image = image::open(path).map_err(|e| format!("Bildet kunne ikke åpnes: {e}"))?;
        if u64::from(image.width()) * u64::from(image.height()) > 60_000_000 {
            return Err("Bildet er for stort. Velg et mindre bilde.".into());
        }
        // Work on a copy no larger than needed.
        let image = if image.height() > 2400 { image.resize(4000, 2400, image::imageops::FilterType::Triangle) } else { image };
        Ok(Edit { original: image.to_rgb8(), zoom: 1.0, x: 0.0, y: -0.3, brightness: 0.0, contrast: 0.0, straighten: 0.0 })
    }

    /// The finished portrait at `height` pixels tall.
    pub fn render(&self, height: u32) -> RgbImage {
        let (w, h) = (self.original.width() as f32, self.original.height() as f32);
        let (mut cw, mut ch) = (w, h);
        if w / h > RATIO { cw = h * RATIO } else { ch = w / RATIO }
        cw /= self.zoom.max(1.0);
        ch /= self.zoom.max(1.0);
        let cx = w / 2.0 + self.x.clamp(-1.0, 1.0) * (w - cw) / 2.0;
        let cy = h / 2.0 + self.y.clamp(-1.0, 1.0) * (h - ch) / 2.0;
        let out_h = height;
        let out_w = (height as f32 * RATIO).round() as u32;
        let scale = ch / out_h as f32;
        let (sin, cos) = self.straighten.to_radians().sin_cos();
        let contrast = 1.0 + self.contrast;
        let bright = self.brightness * 255.0;
        ImageBuffer::from_fn(out_w, out_h, |ox, oy| {
            // Output pixel → rotated position in the source, sampled bilinearly.
            let dx = (ox as f32 - out_w as f32 / 2.0) * scale;
            let dy = (oy as f32 - out_h as f32 / 2.0) * scale;
            let sx = cx + dx * cos - dy * sin;
            let sy = cy + dx * sin + dy * cos;
            let px = sample(&self.original, sx, sy);
            Rgb(px.map(|v| ((v - 128.0) * contrast + 128.0 + bright).clamp(0.0, 255.0) as u8))
        })
    }

    /// A small JPEG for the CV, as the `data:` URL the project file stores.
    pub fn to_data_url(&self) -> Result<String, String> {
        let mut bytes = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 90);
        DynamicImage::ImageRgb8(self.render(OUT_H)).write_with_encoder(encoder).map_err(|e| e.to_string())?;
        Ok(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
    }
}

fn sample(img: &RgbImage, x: f32, y: f32) -> [f32; 3] {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let get = |x: i64, y: i64| -> [f32; 3] {
        if x < 0 || y < 0 || x >= w || y >= h {
            return [255.0; 3]; // white outside the photo, like paper
        }
        let p = img.get_pixel(x as u32, y as u32).0;
        [p[0] as f32, p[1] as f32, p[2] as f32]
    };
    let (a, b, c, d) = (get(x0, y0), get(x0 + 1, y0), get(x0, y0 + 1), get(x0 + 1, y0 + 1));
    std::array::from_fn(|i| (a[i] * (1.0 - fx) + b[i] * fx) * (1.0 - fy) + (c[i] * (1.0 - fx) + d[i] * fx) * fy)
}
