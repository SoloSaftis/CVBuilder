//! Draws the CVBuilder icon (a page with heading lines and a check mark on a blue tile) at the
//! sizes Windows, MSIX, Linux and macOS need, into shared/assets.
//! Run: cargo run -p cvbuilder-app --example make_icons
use image::{
    ExtendedColorType, ImageBuffer, Rgba, RgbaImage,
    codecs::ico::{IcoEncoder, IcoFrame},
};

fn tile(size: u32) -> RgbaImage {
    let s = size as f32;
    let mut img: RgbaImage = ImageBuffer::from_pixel(size, size, Rgba([0, 0, 0, 0]));
    let radius = s * 0.18;
    let inside = |x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32| {
        let cx = x.clamp(x0 + r, x1 - r);
        let cy = y.clamp(y0 + r, y1 - r);
        x >= x0 && x <= x1 && y >= y0 && y <= y1 && ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() <= r
    };
    for (x, y, p) in img.enumerate_pixels_mut() {
        let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
        if inside(fx, fy, 0.0, 0.0, s, s, radius) {
            *p = Rgba([0x24, 0x5a, 0x78, 255]); // tile
        }
        if inside(fx, fy, s * 0.24, s * 0.14, s * 0.76, s * 0.86, s * 0.04) {
            *p = Rgba([255, 255, 255, 255]); // page
            let line = |y0: f32, x1: f32, h: f32| fy >= s * y0 && fy <= s * (y0 + h) && fx >= s * 0.32 && fx <= s * x1;
            if line(0.24, 0.62, 0.06) {
                *p = Rgba([0x24, 0x5a, 0x78, 255]); // name
            } else if line(0.38, 0.68, 0.03) || line(0.46, 0.64, 0.03) || line(0.54, 0.66, 0.03) {
                *p = Rgba([0x9a, 0xab, 0xbb, 255]); // text lines
            }
        }
        // check mark, bottom right
        let (u, v) = (fx / s, fy / s);
        let d1 = ((u - 0.56) - (v - 0.72)).abs() / 1.414; // down stroke
        let d2 = ((u - 0.62) + (v - 0.78)).abs() / 1.414; // up stroke
        let on1 = d1 < 0.035 && (0.50..=0.62).contains(&u) && (0.66..=0.80).contains(&v);
        let on2 = d2 < 0.035 && (0.60..=0.80).contains(&u) && (0.58..=0.80).contains(&v);
        if on1 || on2 {
            *p = Rgba([0x2f, 0x8a, 0x52, 255]);
        }
    }
    img
}

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
    std::fs::create_dir_all(&dir).expect("assets dir");
    let frames: Vec<IcoFrame> = [16, 24, 32, 48, 64, 128, 256].iter().map(|&n| IcoFrame::as_png(tile(n).as_raw(), n, n, ExtendedColorType::Rgba8).expect("frame")).collect();
    let file = std::fs::File::create(dir.join("cvbuilder.ico")).expect("ico");
    IcoEncoder::new(file).encode_images(&frames).expect("encode");
    for n in [44, 50, 150, 256, 512] {
        tile(n).save(dir.join(format!("cvbuilder-{n}.png"))).expect("png");
    }
    println!("icons written to {}", dir.display());
}
