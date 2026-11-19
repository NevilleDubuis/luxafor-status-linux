//! Tray icon pictures: a disc of the status color, or a dashed ring of it
//! while the light is unplugged.

use crate::color::Rgb;

pub const SIZE: u32 = 64;

/// RGBA pixels, 64×64. `away`: dashed ring instead of a disc.
pub fn status_icon(rgb: Rgb, away: bool) -> Vec<u8> {
    let mut px = vec![0u8; (SIZE * SIZE * 4) as usize];
    let c = SIZE as f32 / 2.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let d = (dx * dx + dy * dy).sqrt();
            let alpha = if away {
                // Ring of radius 22, 8 wide, dashes 11 on / 6.3 off (as v1's SVG).
                let ring = (4.5 - (d - 22.0).abs()).clamp(0.0, 1.0);
                let angle = dy.atan2(dx) + std::f32::consts::PI; // 0..2π
                let along = angle * 22.0 % 17.3;
                let dash = if along < 11.0 {
                    (11.0 - along).min(along + 1.0).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                ring * dash
            } else {
                (26.5 - d).clamp(0.0, 1.0)
            };
            let i = ((y * SIZE + x) * 4) as usize;
            px[i..i + 3].copy_from_slice(&rgb);
            px[i + 3] = (alpha * 255.0) as u8;
        }
    }
    px
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disc_and_ring() {
        let disc = status_icon([255, 0, 0], false);
        let alpha = |px: &[u8], x: u32, y: u32| px[((y * SIZE + x) * 4 + 3) as usize];
        assert_eq!(alpha(&disc, 32, 32), 255); // center
        assert_eq!(alpha(&disc, 0, 0), 0); // corner
        let ring = status_icon([255, 0, 0], true);
        assert_eq!(alpha(&ring, 32, 32), 0); // hollow
        let on_ring = (0..SIZE)
            .filter(|x| alpha(&ring, *x, 32) > 0 || alpha(&ring, 32, *x) > 0)
            .count();
        assert!(on_ring > 0);
    }
}
