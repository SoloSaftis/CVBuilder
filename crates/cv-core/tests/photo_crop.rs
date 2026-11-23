//! 168: a portrait that isn't 25:32 is cropped (centred "cover"), never stretched.
use cv_core::document::{cover_crop, jpeg_size};

/// SOI plus an APP0 segment and a baseline start-of-frame of the given pixel size.
fn jpeg_header(w: u16, h: u16) -> Vec<u8> {
    let mut b = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x11, 0x08];
    b.extend(h.to_be_bytes());
    b.extend(w.to_be_bytes());
    b.extend([0x03, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    b
}

/// What is left of the picture after the crop has been taken off, as (width, height).
fn shown(w: u32, h: u32) -> (f32, f32) {
    let c = cover_crop(w, h);
    (w as f32 * (1.0 - c[0] - c[2]), h as f32 * (1.0 - c[1] - c[3]))
}

#[test]
fn reads_the_size() {
    assert_eq!(jpeg_size(&jpeg_header(512, 640)), Some((512, 640)));
    assert_eq!(jpeg_size(b"not a jpeg"), None);
}

#[test]
fn square_loses_the_sides() {
    let c = cover_crop(512, 512);
    assert!(c[1] == 0.0 && c[3] == 0.0 && (c[0] - c[2]).abs() < 1e-6 && c[0] > 0.1);
    let (w, h) = shown(512, 512);
    assert!((w / h - 25.0 / 32.0).abs() < 1e-3);
}

#[test]
fn wide_loses_the_sides() {
    let c = cover_crop(1600, 900);
    assert!(c[1] == 0.0 && c[3] == 0.0 && (c[0] - c[2]).abs() < 1e-6 && c[0] > 0.2);
    let (w, h) = shown(1600, 900);
    assert!((w / h - 25.0 / 32.0).abs() < 1e-3);
}

#[test]
fn tall_loses_top_and_bottom() {
    let c = cover_crop(400, 1000);
    assert!(c[0] == 0.0 && c[2] == 0.0 && (c[1] - c[3]).abs() < 1e-6 && c[1] > 0.2);
    let (w, h) = shown(400, 1000);
    assert!((w / h - 25.0 / 32.0).abs() < 1e-3);
}

#[test]
fn exact_ratio_is_unchanged() {
    assert_eq!(cover_crop(400, 512), [0.0; 4]);
    assert_eq!(cover_crop(512, 655), [0.0; 4]);
    assert_eq!(cover_crop(0, 0), [0.0; 4]);
}
