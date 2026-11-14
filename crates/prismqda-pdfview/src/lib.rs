//! Draws the pages of a PDF as pictures, so a region of a page can be shown and coded.
//!
//! PrismQDA keeps the PDF file as it was imported. A page is drawn only when a window asks to see it, at the width the window
//! wants, and what comes back is a PNG of the page upright (a page turned by the file is drawn the way a reader shows it) on a
//! white background. The rendering is done by `hayro`, a PDF rasterizer written in safe Rust, so a damaged or hostile file cannot
//! take the program down: every call runs on its own thread with a large stack and inside `catch_unwind`, and a page that cannot
//! be drawn is reported as an error, never as a crash.
//!
//! Every position a region of a page is given in is a fraction of the page as drawn (0 is the left or top edge, 1 is the right or
//! bottom edge), so a region keeps its place whatever size the page is drawn at. This crate has no idea what a region is; the
//! engine stores them.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{render, PixmapSettings, RenderCache, RenderSettings};

/// The narrowest and widest a page is ever drawn, in pixels. A request outside is brought inside, so a window cannot ask for a
/// picture too small to see or large enough to use up the computer's memory.
pub const MIN_WIDTH_PX: u32 = 200;
pub const MAX_WIDTH_PX: u32 = 3000;

/// Stack for the thread that reads a PDF. Deeply nested objects in a hostile file would otherwise be able to overflow the
/// caller's stack, which cannot be caught.
const STACK_BYTES: usize = 64 * 1024 * 1024;

/// The size of one page as a reader shows it, in points (1/72 inch), after the page's own turn is applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSize {
    pub width: f64,
    pub height: f64,
}

/// Why a PDF or one of its pages could not be read or drawn. The words are meant for the person to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfViewError(pub String);

impl std::fmt::Display for PdfViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PdfViewError {}

fn damaged() -> PdfViewError {
    PdfViewError(
        "this PDF could not be drawn; it may be damaged, cut short, protected by a password, or saved in a way PrismQDA does not understand yet"
            .to_string(),
    )
}

/// Runs `work` on a thread of its own with a large stack, turning a panic inside the PDF library into an error.
fn guarded<T: Send>(work: impl FnOnce() -> Result<T, PdfViewError> + Send) -> Result<T, PdfViewError> {
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .name("pdf-view".to_string())
            .stack_size(STACK_BYTES)
            .spawn_scoped(scope, move || catch_unwind(AssertUnwindSafe(work)))
            .map_err(|_| PdfViewError("the computer did not have enough memory to draw this PDF".to_string()))?;
        match handle.join() {
            Ok(Ok(result)) => result,
            _ => Err(damaged()),
        }
    })
}

/// The size of every page of the PDF, in order (the first is page 1).
pub fn page_sizes(bytes: &[u8]) -> Result<Vec<PageSize>, PdfViewError> {
    guarded(|| {
        let pdf = Pdf::new(Arc::new(bytes.to_vec())).map_err(|_| damaged())?;
        let pages = pdf.pages();
        let sizes: Vec<PageSize> = pages
            .iter()
            .map(|p| {
                let (w, h) = p.render_dimensions();
                PageSize { width: f64::from(w), height: f64::from(h) }
            })
            .collect();
        if sizes.is_empty() {
            return Err(PdfViewError("this PDF has no pages, or its page list is damaged".to_string()));
        }
        Ok(sizes)
    })
}

/// Where a page sits in its file: how it is turned, and which part of the file's own space is drawn. A place on the page as it is drawn
/// (a fraction of its width and of its height from the top-left, which is how PrismQDA keeps a coded area) and a place in the file's space (points
/// from the bottom-left of the page's media box, which is how the REFI-QDA exchange format names one) are the same place, and this turns one
/// into the other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageGeometry {
    /// The turn the file gives the page, in degrees clockwise: 0, 90, 180, or 270.
    pub rotation: u16,
    /// The part of the page that is drawn (the crop box inside the media box) and the media box, as `(x0, y0, x1, y1)` in points, y up.
    pub drawn: (f64, f64, f64, f64),
    pub media: (f64, f64, f64, f64),
}

impl PageGeometry {
    fn sizes(&self) -> (f64, f64) {
        (self.drawn.2 - self.drawn.0, self.drawn.3 - self.drawn.1)
    }

    /// The size of the page as it is drawn, upright, in points.
    pub fn drawn_size(&self) -> PageSize {
        let (w, h) = self.sizes();
        if matches!(self.rotation, 90 | 270) {
            PageSize { width: h, height: w }
        } else {
            PageSize { width: w, height: h }
        }
    }

    /// A place on the page as drawn, as fractions from its top-left, as a point in points from the bottom-left of the media box.
    pub fn to_media_points(&self, fx: f64, fy: f64) -> (f64, f64) {
        let (cw, ch) = self.sizes();
        // (u, v): from the left and from the top of the page as drawn, in points
        let (x_rel, y_rel) = match self.rotation {
            90 => {
                let (u, v) = (fx * ch, fy * cw);
                (v, u)
            }
            180 => {
                let (u, v) = (fx * cw, fy * ch);
                (cw - u, v)
            }
            270 => {
                let (u, v) = (fx * ch, fy * cw);
                (cw - v, ch - u)
            }
            _ => {
                let (u, v) = (fx * cw, fy * ch);
                (u, ch - v)
            }
        };
        (self.drawn.0 + x_rel - self.media.0, self.drawn.1 + y_rel - self.media.1)
    }

    /// The other way: a point in points from the bottom-left of the media box as fractions of the page as drawn from its top-left. Not
    /// kept inside the page: a point outside it gives a fraction below 0 or above 1.
    pub fn from_media_points(&self, x: f64, y: f64) -> (f64, f64) {
        let (cw, ch) = self.sizes();
        let (x_rel, y_rel) = (x + self.media.0 - self.drawn.0, y + self.media.1 - self.drawn.1);
        match self.rotation {
            90 => (y_rel / ch, x_rel / cw),
            180 => ((cw - x_rel) / cw, y_rel / ch),
            270 => ((ch - y_rel) / ch, (cw - x_rel) / cw),
            _ => (x_rel / cw, (ch - y_rel) / ch),
        }
    }
}

/// How every page of the PDF sits in its file, in order (the first is page 1).
pub fn page_geometries(bytes: &[u8]) -> Result<Vec<PageGeometry>, PdfViewError> {
    guarded(|| {
        let pdf = Pdf::new(Arc::new(bytes.to_vec())).map_err(|_| damaged())?;
        let pages = pdf.pages();
        let out: Vec<PageGeometry> = pages
            .iter()
            .map(|p| {
                let media = p.media_box();
                let drawn = p.intersected_crop_box();
                let (w, h) = p.base_dimensions();
                // a page with no area is drawn as an A4 page (the viewer's own rule); the part drawn is then that size from the corner
                let drawn = if drawn.width() < 1e-3 || drawn.height() < 1e-3 { (media.x0, media.y0, media.x0 + f64::from(w), media.y0 + f64::from(h)) } else { (drawn.x0, drawn.y0, drawn.x1, drawn.y1) };
                let rotation = match p.rotation() {
                    hayro::hayro_syntax::page::Rotation::None => 0,
                    hayro::hayro_syntax::page::Rotation::Horizontal => 90,
                    hayro::hayro_syntax::page::Rotation::Flipped => 180,
                    hayro::hayro_syntax::page::Rotation::FlippedHorizontal => 270,
                };
                PageGeometry { rotation, drawn, media: (media.x0, media.y0, media.x1, media.y1) }
            })
            .collect();
        if out.is_empty() {
            return Err(PdfViewError("this PDF has no pages, or its page list is damaged".to_string()));
        }
        Ok(out)
    })
}

/// A PNG of page `page` (counting from 1), `width_px` pixels wide (kept between [`MIN_WIDTH_PX`] and [`MAX_WIDTH_PX`]) and as tall
/// as the page's shape makes it.
pub fn render_page_png(bytes: &[u8], page: u32, width_px: u32) -> Result<Vec<u8>, PdfViewError> {
    if page == 0 {
        return Err(PdfViewError("pages are counted from 1".to_string()));
    }
    let width_px = width_px.clamp(MIN_WIDTH_PX, MAX_WIDTH_PX);
    guarded(|| {
        let pdf = Pdf::new(Arc::new(bytes.to_vec())).map_err(|_| damaged())?;
        let pages = pdf.pages();
        let Some(p) = pages.get(page as usize - 1) else {
            return Err(PdfViewError(format!("this PDF has {} pages, so there is no page {page}", pages.len())));
        };
        let (w, h) = p.render_dimensions();
        if !(w.is_finite() && h.is_finite() && w >= 1.0 && h >= 1.0) {
            return Err(PdfViewError(format!("page {page} has no usable size")));
        }
        // Scale so the drawn page is `width_px` wide; the height follows, and is kept inside what the renderer can hold (65535).
        let mut scale = width_px as f32 / w;
        if h * scale > 20000.0 {
            scale = 20000.0 / h;
        }
        let cache = RenderCache::new();
        let pixmap = render(
            p,
            &cache,
            &InterpreterSettings::default(),
            &RenderSettings::default(),
            &PixmapSettings { x_scale: scale, y_scale: scale, bg_color: WHITE },
        );
        pixmap.into_png().map_err(|_| PdfViewError(format!("page {page} could not be turned into a picture")))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three_pages() -> Vec<u8> {
        let path = format!("{}/../prismqda-import/tests/fixtures/pdf/three_pages.pdf", env!("CARGO_MANIFEST_DIR"));
        std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
    }

    #[test]
    fn every_page_has_a_size() {
        let sizes = page_sizes(&three_pages()).unwrap();
        assert_eq!(sizes.len(), 3);
        for s in &sizes {
            assert!(s.width > 100.0 && s.height > 100.0, "{s:?}");
        }
    }

    #[test]
    fn a_page_is_drawn_at_the_width_asked_for() {
        let png = render_page_png(&three_pages(), 1, 600).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
        let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
        assert_eq!(width, 600);
        let size = page_sizes(&three_pages()).unwrap()[0];
        let expected = (600.0 * size.height / size.width).round() as i64;
        assert!((i64::from(height) - expected).abs() <= 1, "{height} against {expected}");
    }

    #[test]
    fn a_page_that_is_not_there_is_a_sentence_not_a_crash() {
        let e = render_page_png(&three_pages(), 4, 600).unwrap_err();
        assert!(e.to_string().contains("3 pages"), "{e}");
        assert!(render_page_png(&three_pages(), 0, 600).is_err());
    }

    #[test]
    fn something_that_is_not_a_pdf_is_a_sentence_not_a_crash() {
        assert!(page_sizes(b"this is not a pdf").is_err());
        assert!(render_page_png(&[], 1, 600).is_err());
        assert!(render_page_png(b"%PDF-1.4\nnonsense", 1, 600).is_err());
    }

    /// A one-page PDF whose page is `media`, turned `rotate` degrees, with a black square of 20 by 20 points at (10, 10) in the file's own space.
    fn one_page(media: [f64; 4], crop: Option<[f64; 4]>, rotate: u16) -> Vec<u8> {
        let crop = crop.map(|c| format!(" /CropBox [{} {} {} {}]", c[0], c[1], c[2], c[3])).unwrap_or_default();
        let content = "0 0 0 rg 10 10 20 20 re f";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [{} {} {} {}]{crop} /Rotate {rotate} /Contents 4 0 R /Resources << >> >>", media[0], media[1], media[2], media[3]),
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        ];
        let mut out = b"%PDF-1.4\n".to_vec();
        let mut at = Vec::new();
        for (i, o) in objects.iter().enumerate() {
            at.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
        for a in &at {
            out.extend_from_slice(format!("{a:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
        out
    }

    /// Where the black square is on the page as drawn: (left, top, right, bottom) as fractions.
    fn square_on_the_page(pdf: &[u8]) -> (f64, f64, f64, f64) {
        let png = render_page_png(pdf, 1, 600).unwrap();
        let picture = image::load_from_memory(&png).unwrap().to_rgba8();
        let (w, h) = (f64::from(picture.width()), f64::from(picture.height()));
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
        for (x, y, p) in picture.enumerate_pixels() {
            if p[3] > 200 && u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2]) < 150 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
        assert!(x1 > x0 && y1 > y0, "the square was drawn");
        (f64::from(x0) / w, f64::from(y0) / h, f64::from(x1) / w, f64::from(y1) / h)
    }

    #[test]
    fn a_place_on_the_page_as_drawn_is_the_same_place_in_the_files_space_whatever_the_turn_or_the_crop() {
        // The black square is at (10, 10) to (30, 30) in the file's space, y up from the bottom-left of the media box. Drawn, it must be where
        // the geometry says that is, for a page turned 0, 90, 180, and 270 degrees, with and without a crop box that starts away from the corner.
        for (crop, rotate) in [(None, 0), (None, 90), (None, 180), (None, 270), (Some([5.0, 5.0, 150.0, 90.0]), 0), (Some([5.0, 5.0, 150.0, 90.0]), 90), (Some([5.0, 5.0, 150.0, 90.0]), 270)] {
            let pdf = one_page([0.0, 0.0, 200.0, 100.0], crop, rotate);
            let g = page_geometries(&pdf).unwrap()[0];
            assert_eq!(g.rotation, rotate, "{crop:?} {rotate}");
            let size = page_sizes(&pdf).unwrap()[0];
            assert!((g.drawn_size().width - size.width).abs() < 1e-6 && (g.drawn_size().height - size.height).abs() < 1e-6, "the drawn size agrees with the viewer's");
            let (l, t, r, b) = square_on_the_page(&pdf);
            // the corners of what was drawn, in the file's space: the square's bottom-left is (10, 10) and its top-right is (30, 30)
            let corners = [g.to_media_points(l, t), g.to_media_points(r, t), g.to_media_points(l, b), g.to_media_points(r, b)];
            let xs: Vec<f64> = corners.iter().map(|c| c.0).collect();
            let ys: Vec<f64> = corners.iter().map(|c| c.1).collect();
            let (min_x, max_x) = (xs.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max));
            let (min_y, max_y) = (ys.iter().cloned().fold(f64::MAX, f64::min), ys.iter().cloned().fold(f64::MIN, f64::max));
            for (got, want, what) in [(min_x, 10.0, "left"), (max_x, 30.0, "right"), (min_y, 10.0, "bottom"), (max_y, 30.0, "top")] {
                assert!((got - want).abs() < 1.5, "{crop:?} turned {rotate}: the {what} edge is at {got}, not {want}");
            }
            // and back again
            let (fx, fy) = g.from_media_points(g.to_media_points(0.25, 0.75).0, g.to_media_points(0.25, 0.75).1);
            assert!((fx - 0.25).abs() < 1e-9 && (fy - 0.75).abs() < 1e-9, "{crop:?} turned {rotate}: {fx}, {fy}");
        }
    }

    #[test]
    fn the_width_asked_for_is_kept_inside_what_is_sensible() {
        let png = render_page_png(&three_pages(), 1, 5).unwrap();
        assert_eq!(u32::from_be_bytes([png[16], png[17], png[18], png[19]]), MIN_WIDTH_PX);
    }

    #[test]
    fn text_is_drawn_dark_on_white() {
        // The first page has a title and paragraphs, so a page drawn at a readable width has some dark pixels and a white margin.
        let png = render_page_png(&three_pages(), 1, 600).unwrap();
        let picture = image::load_from_memory(&png).unwrap().to_rgba8();
        let dark = picture.pixels().filter(|p| p[3] > 200 && u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2]) < 200).count();
        assert!(dark > 500, "some ink: {dark}");
        assert_eq!(picture.get_pixel(0, 0).0, [255, 255, 255, 255], "the corner of the page is paper");
    }
}
