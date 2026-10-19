//! The application mark, as pixels: an indigo tile and two arcs reading
//! "cc", coral then lilac, each over a faded copy of itself shifted down and
//! right.
//!
//! Rasterised here from the same geometry as `LogoMark` in `view` rather than
//! shipped as an image file. One drawing, one source of truth: the mark in the
//! header, the window's icon, the one inside the `.exe` (`build.rs`) and the
//! installers' files all come from this function. Plain Rust and nothing
//! else, so the build script can include it.

/// The tile: Minuit's card. A logo does not follow the theme, but this one is
/// drawn from the default theme's colours.
pub const TILE: [u8; 3] = [0x16, 0x13, 0x31];
pub const CORAL: [u8; 3] = [0xFF, 0x7A, 0x66];
pub const LILAC: [u8; 3] = [0xC9, 0xA2, 0xFF];
/// Corner radius, ring radius and half the stroke, as fractions of the side.
pub const CORNER: f32 = 0.27;
pub const RING: f32 = 0.15;
pub const HALF_STROKE: f32 = 0.0475;
/// Each "c", back to front: its centre as fractions of the side, its ink and
/// its opacity. The two faded copies come first, offset down and right: the
/// mark copies itself.
pub const ARCS: [((f32, f32), [u8; 3], f32); 4] = [
    ((0.37, 0.54), CORAL, 0.4),
    ((0.70, 0.54), LILAC, 0.4),
    ((0.31, 0.48), CORAL, 1.0),
    ((0.64, 0.48), LILAC, 1.0),
];
/// Samples per axis inside each pixel. Three is enough to take the staircase
/// off a 64-pixel tile, and this runs once at startup.
const SAMPLES: u32 = 3;

/// The icon as RGBA rows, `size` by `size` pixels.
pub fn rgba(size: u32) -> Vec<u8> {
    let n = size as f32;
    let (radius, ring, half_stroke) = (CORNER * n, RING * n, HALF_STROKE * n);
    let channel = |c: [u8; 3]| c.map(f32::from);

    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (mut coverage, mut sum) = (0.0, [0.0f32; 3]);
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let px = x as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                    let py = y as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
                    if !inside_rounded_square(px, py, n, radius) {
                        continue;
                    }
                    coverage += 1.0;
                    // Painted back to front, each arc blended over what is
                    // already there.
                    let mut colour = channel(TILE);
                    for &((cx, cy), ink, opacity) in &ARCS {
                        if on_arc(px - cx * n, py - cy * n, ring, half_stroke) {
                            let ink = channel(ink);
                            colour = [0, 1, 2].map(|i| lerp(colour[i], ink[i], opacity));
                        }
                    }
                    for i in 0..3 {
                        sum[i] += colour[i];
                    }
                }
            }
            if coverage == 0.0 {
                pixels.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            let total = (SAMPLES * SAMPLES) as f32;
            pixels.extend_from_slice(&[
                (sum[0] / coverage) as u8,
                (sum[1] / coverage) as u8,
                (sum[2] / coverage) as u8,
                (255.0 * coverage / total) as u8,
            ]);
        }
    }
    pixels
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

fn inside_rounded_square(x: f32, y: f32, side: f32, radius: f32) -> bool {
    // Distance to the rounded rectangle, measured from the inner box whose
    // corners the radius rounds off.
    let dx = (radius - x).max(x - (side - radius)).max(0.0);
    let dy = (radius - y).max(y - (side - radius)).max(0.0);
    dx * dx + dy * dy <= radius * radius
}

/// A "c": the ring, minus the quarter open to the right, with rounded ends.
fn on_arc(dx: f32, dy: f32, ring: f32, half_stroke: f32) -> bool {
    let distance = (dx * dx + dy * dy).sqrt();
    if (distance - ring).abs() > half_stroke {
        return false;
    }
    let open = dx > 0.0 && dy.abs() <= dx;
    if !open {
        return true;
    }
    // Inside the opening only the two round caps remain.
    let cap = ring * std::f32::consts::FRAC_1_SQRT_2;
    [(cap, -cap), (cap, cap)]
        .iter()
        .any(|&(ex, ey)| ((dx - ex).powi(2) + (dy - ey).powi(2)).sqrt() <= half_stroke)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(pixels: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * size + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    }

    #[test]
    fn the_tile_is_rounded_and_carries_both_inks() {
        const SIZE: u32 = 64;
        let pixels = rgba(SIZE);
        assert_eq!(pixels.len(), (SIZE * SIZE * 4) as usize);

        // The corner is cut away by the rounding.
        assert_eq!(pixel(&pixels, SIZE, 0, 0)[3], 0, "corner must be clear");
        // The middle of an edge is inside, opaque, and the tile's colour.
        let edge = pixel(&pixels, SIZE, SIZE / 2, 2);
        assert_eq!(edge, [TILE[0], TILE[1], TILE[2], 255], "edge is solid tile");

        // Each front "c" shows its ink at full strength somewhere.
        let has = |ink: [u8; 3]| {
            (0..SIZE)
                .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
                .any(|(x, y)| pixel(&pixels, SIZE, x, y) == [ink[0], ink[1], ink[2], 255])
        };
        assert!(has(CORAL), "the first c must be coral");
        assert!(has(LILAC), "the second c must be lilac");
    }
}
