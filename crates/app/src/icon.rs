//! The window icon: what the taskbar, Alt-Tab and the window list show.
//!
//! Rasterised here from the same geometry as `LogoMark` in `view` — a rounded
//! tile, magenta fading to violet, and two white arcs reading "cc" — rather
//! than shipped as an image file. One drawing, one source of truth: the mark in
//! the header and the mark in the taskbar cannot drift apart. The same function
//! will produce the `.ico` and `.icns` the installers need.

/// Brand colours, as in `LogoMark`. A logo does not follow the theme.
const START: [f32; 3] = [0xF0 as f32, 0x00 as f32, 0xA0 as f32];
const END: [f32; 3] = [0x8A as f32, 0x2B as f32, 0xC2 as f32];
/// Samples per axis inside each pixel. Three is enough to take the staircase
/// off a 64-pixel tile, and this runs once at startup.
const SAMPLES: u32 = 3;

/// The icon as RGBA rows, `size` by `size` pixels.
pub fn rgba(size: u32) -> Vec<u8> {
    let n = size as f32;
    let radius = 0.27 * n;
    let ring = 0.15 * n;
    let half_stroke = 0.0475 * n;
    let centres = [(0.33 * n, 0.5 * n), (0.66 * n, 0.5 * n)];

    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (mut coverage, mut ink, mut mix) = (0.0, 0.0, 0.0);
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let px = x as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                    let py = y as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
                    if !inside_rounded_square(px, py, n, radius) {
                        continue;
                    }
                    coverage += 1.0;
                    mix += (px + py) / (2.0 * n);
                    if centres
                        .iter()
                        .any(|&(cx, cy)| on_arc(px - cx, py - cy, ring, half_stroke))
                    {
                        ink += 1.0;
                    }
                }
            }
            let total = (SAMPLES * SAMPLES) as f32;
            if coverage == 0.0 {
                pixels.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            let mix = mix / coverage;
            let ink = ink / coverage;
            let tile = [
                lerp(START[0], END[0], mix),
                lerp(START[1], END[1], mix),
                lerp(START[2], END[2], mix),
            ];
            let colour = [
                lerp(tile[0], 255.0, ink),
                lerp(tile[1], 255.0, ink),
                lerp(tile[2], 255.0, ink),
            ];
            pixels.extend_from_slice(&[
                colour[0] as u8,
                colour[1] as u8,
                colour[2] as u8,
                (255.0 * coverage / total) as u8,
            ]);
        }
    }
    pixels
}

/// The icon iced hands to the window system. `None` rather than a panic: a
/// window without its mark is a blemish, not a reason to refuse to start.
pub fn window() -> Option<iced::window::Icon> {
    const SIZE: u32 = 64;
    iced::window::icon::from_rgba(rgba(SIZE), SIZE, SIZE).ok()
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
    fn the_tile_is_rounded_coloured_and_carries_white_ink() {
        const SIZE: u32 = 64;
        let pixels = rgba(SIZE);
        assert_eq!(pixels.len(), (SIZE * SIZE * 4) as usize);

        // The corner is cut away by the rounding.
        assert_eq!(pixel(&pixels, SIZE, 0, 0)[3], 0, "corner must be clear");
        // The middle of an edge is inside, and opaque.
        assert_eq!(pixel(&pixels, SIZE, SIZE / 2, 2)[3], 255, "edge is solid");

        // Somewhere on the left "c", the stroke is near-white.
        let on_c = (0..SIZE)
            .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
            .map(|(x, y)| pixel(&pixels, SIZE, x, y))
            .any(|p| p[0] > 230 && p[1] > 230 && p[2] > 230 && p[3] == 255);
        assert!(on_c, "the arcs must be drawn in white");
    }
}
