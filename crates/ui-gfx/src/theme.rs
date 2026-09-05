//! The colour palette and metrics for the menu (report §3.10.1). One cohesive
//! place for visual constants (SRP) — no logic.
use crate::canvas::Rgb;

pub const BG: Rgb = Rgb::new(0x0e, 0x12, 0x1a);          // near-black slate
pub const PANEL: Rgb = Rgb::new(0x18, 0x20, 0x2c);
pub const TEXT: Rgb = Rgb::new(0xe6, 0xed, 0xf3);
pub const TEXT_DIM: Rgb = Rgb::new(0x8a, 0x96, 0xa6);
pub const ACCENT: Rgb = Rgb::new(0x4c, 0x9a, 0xff);       // selection highlight
pub const OK: Rgb = Rgb::new(0x4c, 0xd9, 0x64);
pub const WARN: Rgb = Rgb::new(0xf0, 0xc0, 0x36);
pub const FAIL: Rgb = Rgb::new(0xf0, 0x5c, 0x5c);

pub const ROW_HEIGHT: usize = 28;
pub const MARGIN: usize = 40;
pub const GLYPH_SCALE: usize = 2; // 8x8 font drawn at 2x → 16px tall
