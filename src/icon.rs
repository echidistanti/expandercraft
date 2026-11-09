//! The application icon, drawn procedurally (used for the window, the tray, and the Windows executable).
//! This file is also included by `build.rs` to generate the `.ico` resource.

/// Render the icon as RGBA pixels. `active == false` renders the grayscale "paused" variant.
pub fn render_rgba(size: u32, active: bool) -> Vec<u8> {
    const SS: u32 = 4; // supersampling factor
    let n = size as f32;
    let mut out = vec![0u8; (size * size * 4) as usize];
    // geometry in normalized coordinates
    let radius = 0.23;
    let stroke = 0.075;
    let chevron = [((0.25, 0.30), (0.45, 0.50)), ((0.45, 0.50), (0.25, 0.70))];
    let bars = [((0.57, 0.38), (0.77, 0.38)), ((0.57, 0.62), (0.69, 0.62))];
    for py in 0..size {
        for px in 0..size {
            let (mut bg_cov, mut fg_cov) = (0.0f32, 0.0f32);
            let mut top_mix = 0.0f32;
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (px as f32 + (sx as f32 + 0.5) / SS as f32) / n;
                    let y = (py as f32 + (sy as f32 + 0.5) / SS as f32) / n;
                    if in_rounded_rect(x, y, 0.04, 0.96, radius) {
                        bg_cov += 1.0;
                        top_mix += 1.0 - y;
                        let on_glyph = chevron.iter().chain(bars.iter()).any(|(a, b)| dist_to_segment(x, y, *a, *b) < stroke);
                        if on_glyph {
                            fg_cov += 1.0;
                        }
                    }
                }
            }
            let samples = (SS * SS) as f32;
            if bg_cov == 0.0 {
                continue;
            }
            let t = top_mix / bg_cov; // 1 at the top, 0 at the bottom
            let (top, bottom) = if active { ([0x2D, 0xD4, 0xBF], [0x0F, 0x76, 0x6E]) } else { ([0xA8, 0xAE, 0xB5], [0x5B, 0x63, 0x6B]) };
            let bg: Vec<f32> = (0..3).map(|i| bottom[i] as f32 + (top[i] as f32 - bottom[i] as f32) * t).collect();
            let fg_ratio = fg_cov / bg_cov;
            let i = ((py * size + px) * 4) as usize;
            for c in 0..3 {
                out[i + c] = (bg[c] * (1.0 - fg_ratio) + 255.0 * fg_ratio).round() as u8;
            }
            out[i + 3] = (255.0 * bg_cov / samples).round() as u8;
        }
    }
    out
}

fn in_rounded_rect(x: f32, y: f32, lo: f32, hi: f32, r: f32) -> bool {
    if x < lo || x > hi || y < lo || y > hi {
        return false;
    }
    let cx = x.clamp(lo + r, hi - r);
    let cy = y.clamp(lo + r, hi - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

fn dist_to_segment(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((x - a.0) * dx + (y - a.1) * dy) / len2).clamp(0.0, 1.0) };
    let (px, py) = (a.0 + t * dx, a.1 + t * dy);
    ((x - px).powi(2) + (y - py).powi(2)).sqrt()
}

/// Build a Windows `.ico` file containing the given sizes (32-bit BMP entries).
#[allow(dead_code)]
pub fn build_ico(sizes: &[u32]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&s| {
            let rgba = render_rgba(s, true);
            let mut bmp = Vec::new();
            // BITMAPINFOHEADER (height is doubled: XOR mask + AND mask)
            bmp.extend_from_slice(&40u32.to_le_bytes());
            bmp.extend_from_slice(&(s as i32).to_le_bytes());
            bmp.extend_from_slice(&((s * 2) as i32).to_le_bytes());
            bmp.extend_from_slice(&1u16.to_le_bytes());
            bmp.extend_from_slice(&32u16.to_le_bytes());
            bmp.extend_from_slice(&0u32.to_le_bytes());
            bmp.extend_from_slice(&(s * s * 4).to_le_bytes());
            bmp.extend_from_slice(&[0u8; 16]);
            for y in (0..s).rev() {
                for x in 0..s {
                    let i = ((y * s + x) * 4) as usize;
                    bmp.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
                }
            }
            let mask_row = s.div_ceil(32) * 4;
            bmp.extend(std::iter::repeat_n(0u8, (mask_row * s) as usize));
            bmp
        })
        .collect();
    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (s, img) in sizes.iter().zip(&images) {
        let dim = if *s >= 256 { 0u8 } else { *s as u8 };
        ico.extend_from_slice(&[dim, dim, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(img.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in images {
        ico.extend_from_slice(&img);
    }
    ico
}
