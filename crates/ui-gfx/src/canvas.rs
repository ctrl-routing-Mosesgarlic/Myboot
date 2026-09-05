//! The drawing surface abstraction (report §4.5). The renderer targets this
//! trait, not uefi, so it is host-testable. `platform::Framebuffer` implements it
//! (in the engine) for real graphics; tests implement it with a pixel buffer.
extern crate alloc;

/// An 8-bit-per-channel RGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb { pub r: u8, pub g: u8, pub b: u8 }
impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self { Rgb { r, g, b } }
}

/// A minimal 2D surface. Implementations must bounds-check: out-of-range writes
/// are ignored, never a panic (the renderer relies on this for safety).
pub trait Canvas {
    fn dimensions(&self) -> (usize, usize);
    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb);
    /// Fill a rectangle; default is a per-pixel loop over `put_pixel`.
    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Rgb) {
        let (cw, ch) = self.dimensions();
        for yy in y..y.saturating_add(h).min(ch) {
            for xx in x..x.saturating_add(w).min(cw) {
                self.put_pixel(xx, yy, color);
            }
        }
    }
}
