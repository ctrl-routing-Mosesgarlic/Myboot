//! Draw the menu to any `Canvas` (report §4.5). Pure w.r.t. firmware: it only
//! calls `Canvas` methods, so it is host-tested against a pixel-buffer canvas.
extern crate alloc;
use crate::canvas::{Canvas, Rgb};
use crate::menu::{Menu, HealthTag};
use crate::{theme, font};

/// Render a full frame: background, title, and one row per entry with a health
/// dot and the selection highlight.
pub fn draw(canvas: &mut dyn Canvas, menu: &Menu, title: &str) {
    let (w, _h) = canvas.dimensions();
    canvas.fill_rect(0, 0, w, canvas.dimensions().1, theme::BG);

    // Title bar
    draw_text(canvas, theme::MARGIN, theme::MARGIN, title, theme::TEXT);

    let list_top = theme::MARGIN + theme::ROW_HEIGHT;
    for (i, row) in menu.rows().iter().enumerate() {
        let y = list_top + i * theme::ROW_HEIGHT;
        // selection highlight
        if i == menu.cursor() {
            canvas.fill_rect(theme::MARGIN - 8, y - 4, w - 2 * (theme::MARGIN - 8),
                             theme::ROW_HEIGHT, theme::PANEL);
            canvas.fill_rect(theme::MARGIN - 8, y - 4, 4, theme::ROW_HEIGHT, theme::ACCENT);
        }
        // health dot
        let dot = match row.health {
            HealthTag::Ok => theme::OK, HealthTag::Warn => theme::WARN,
            HealthTag::Fail => theme::FAIL, HealthTag::Unknown => theme::TEXT_DIM,
        };
        canvas.fill_rect(theme::MARGIN, y + 4, 10, 10, dot);
        // title text (dimmed if not bootable)
        let color = if row.bootable { theme::TEXT } else { theme::TEXT_DIM };
        draw_text(canvas, theme::MARGIN + 22, y, &row.title, color);
    }
}

/// Draw a string at (x, y) using the 8x8 font scaled by `theme::GLYPH_SCALE`.
/// Bounds-checking is delegated to the canvas (out-of-range pixels are ignored).
pub fn draw_text(canvas: &mut dyn Canvas, x: usize, y: usize, text: &str, color: Rgb) {
    let scale = theme::GLYPH_SCALE;
    let mut cx = x;
    for ch in text.chars() {
        let g = font::glyph(ch);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..font::GLYPH_W {
                if bits & (1 << col) != 0 {
                    // draw a scale×scale block for this set bit
                    let px = cx + col * scale;
                    let py = y + row * scale;
                    canvas.fill_rect(px, py, scale, scale, color);
                }
            }
        }
        cx += (font::GLYPH_W + 1) * scale; // 1px inter-glyph gap
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{Canvas, Rgb};
    use crate::menu::Menu;
    use alloc::vec::Vec;
    use graph::{BootGraph, DiskNode, DiskId, OsNode, OsKind, EntryRole, BootMethod, BootEntry, Health};
    use alloc::string::ToString;

    /// A pixel-buffer canvas for testing the renderer without firmware.
    struct BufCanvas { w: usize, h: usize, px: Vec<Rgb> }
    impl BufCanvas { fn new(w: usize, h: usize) -> Self { BufCanvas { w, h, px: alloc::vec![Rgb::new(0,0,0); w*h] } } }
    impl Canvas for BufCanvas {
        fn dimensions(&self) -> (usize, usize) { (self.w, self.h) }
        fn put_pixel(&mut self, x: usize, y: usize, c: Rgb) {
            if x < self.w && y < self.h { self.px[y*self.w + x] = c; }
        }
    }

    #[test]
    fn out_of_range_text_never_panics() {
        let mut c = BufCanvas::new(64, 32);
        // draw far outside the canvas — must be clamped, not panic
        draw_text(&mut c, 1000, 1000, "overflow", Rgb::new(255,255,255));
        draw_text(&mut c, 60, 28, "edge case that runs off the side", theme::TEXT);
    }

    #[test]
    fn draw_sets_some_pixels_for_a_menu() {
        let mut g = BootGraph::new();
        let mut os = OsNode::new(OsKind::NixOs, "NixOS");
        let mut e = BootEntry::new(OsKind::NixOs, "NixOS gen 1", EntryRole::Generation(1), BootMethod::NixGeneration);
        e.health = Health::Healthy;
        os.push(e);
        g.add_disk(DiskNode { id: DiskId(0), label: "d".to_string(), systems: alloc::vec![os] });
        let menu = Menu::from_graph(&g);

        let mut c = BufCanvas::new(400, 200);
        draw(&mut c, &menu, "MyBoot");
        // some non-background pixels must have been drawn (text/dots/highlight)
        let non_bg = c.px.iter().filter(|p| **p != theme::BG).count();
        assert!(non_bg > 50, "expected the menu to render visible content");
    }

    #[test]
    fn glyph_bit_maps_to_a_pixel_block() {
        // 'A' (0x41) has its top row bit pattern 0x0C = bits 2,3 set.
        let mut c = BufCanvas::new(64, 64);
        draw_text(&mut c, 0, 0, "A", Rgb::new(255,255,255));
        // with scale 2, bit col 2 → x in [4,6); row 0 → y in [0,2)
        let idx = 0 * c.w + 4;
        assert_eq!(c.px[idx], Rgb::new(255,255,255));
    }
}
