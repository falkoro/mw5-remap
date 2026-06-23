//! Image export for the device sheet: write the captured device-panel image as a
//! PNG and as a single-page PDF that embeds the image as a JPEG `/DCTDecode`
//! XObject (so no compression code is needed — the raw JPEG bytes go straight
//! into the PDF stream). Both take an `image::RgbaImage` cropped from the egui
//! screenshot.

use image::{ImageEncoder, RgbaImage};
use std::io::Write;
use std::path::Path;

/// Encode the image as a PNG file at `path`.
pub fn write_png(img: &RgbaImage, path: &Path) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let w = std::io::BufWriter::new(file);
    let enc = image::codecs::png::PngEncoder::new(w);
    enc.write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

/// Encode the image as a single-page PDF at `path`. The page embeds the image as
/// a baseline JPEG via a `/DCTDecode` image XObject, sized 1px = 1pt.
pub fn write_pdf(img: &RgbaImage, path: &Path) -> std::io::Result<()> {
    let (w, h) = (img.width(), img.height());

    // 1) Encode the RGBA image to baseline JPEG (DCTDecode wants DeviceRGB).
    let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let mut jpeg: Vec<u8> = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90)
        .write_image(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // 2) Assemble the PDF. We track each object's byte offset for the xref table.
    let mut pdf: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = Vec::new(); // offsets[i] = byte offset of object (i+1)

    macro_rules! obj {
        ($body:expr) => {{
            offsets.push(pdf.len());
            pdf.extend_from_slice($body);
        }};
    }

    pdf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    // Object 1: Catalog
    obj!(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // Object 2: Pages
    obj!(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    // Object 3: Page — MediaBox is the image size in points (1px = 1pt).
    let page = format!(
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] \
/Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>\nendobj\n"
    );
    obj!(page.as_bytes());

    // Object 4: the image XObject (DCTDecode = embedded JPEG).
    let img_hdr = format!(
        "4 0 obj\n<< /Type /XObject /Subtype /Image /Width {w} /Height {h} \
/ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
        jpeg.len()
    );
    offsets.push(pdf.len());
    pdf.extend_from_slice(img_hdr.as_bytes());
    pdf.extend_from_slice(&jpeg);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");

    // Object 5: content stream — scale the unit image XObject to fill the page.
    let content = format!("q\n{w} 0 0 {h} 0 0 cm\n/Im0 Do\nQ\n");
    let stream5 = format!(
        "5 0 obj\n<< /Length {} >>\nstream\n{content}endstream\nendobj\n",
        content.len()
    );
    obj!(stream5.as_bytes());

    // 3) xref table + trailer.
    let xref_pos = pdf.len();
    let n = offsets.len(); // number of real objects (1..=n)
    let mut xref = format!("xref\n0 {}\n0000000000 65535 f \n", n + 1);
    for off in &offsets {
        xref.push_str(&format!("{off:010} 00000 n \n"));
    }
    pdf.extend_from_slice(xref.as_bytes());

    let trailer = format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n",
        n + 1
    );
    pdf.extend_from_slice(trailer.as_bytes());

    let mut f = std::fs::File::create(path)?;
    f.write_all(&pdf)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pdf_and_png_are_valid() {
        let mut img = image::RgbaImage::new(40, 30);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba([(x * 6) as u8, (y * 8) as u8, 128, 255]);
        }
        let dir = std::env::temp_dir();
        let png = dir.join("mw5_test_sheet.png");
        let pdf = dir.join("mw5_test_sheet.pdf");
        write_png(&img, &png).unwrap();
        write_pdf(&img, &pdf).unwrap();

        // PNG decodes back to the same dimensions.
        let back = image::open(&png).unwrap();
        assert_eq!((back.width(), back.height()), (40, 30));

        // PDF structural checks.
        let bytes = std::fs::read(&pdf).unwrap();
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(bytes.ends_with(b"%%EOF\n"));
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("/MediaBox [0 0 40 30]"));
        assert!(s.contains("/DCTDecode"));
        assert!(s.contains("startxref"));

        // Verify each xref offset points at the right "N 0 obj".
        let xref_at = s.rfind("\nxref\n").unwrap() + 1;
        let after = &s[xref_at..];
        let mut lines = after.lines();
        assert_eq!(lines.next().unwrap(), "xref");
        let header = lines.next().unwrap(); // "0 6"
        let count: usize = header.split(' ').nth(1).unwrap().parse().unwrap();
        // first entry is the free object 0
        let _free = lines.next().unwrap();
        for i in 1..count {
            let line = lines.next().unwrap();
            let off: usize = line[..10].parse().unwrap();
            let expect = format!("{i} 0 obj");
            assert!(bytes[off..].starts_with(expect.as_bytes()),
                "object {i} xref offset {off} does not point at '{expect}'");
        }
        // startxref value points at the 'xref' keyword.
        let sx = s.rfind("startxref\n").unwrap();
        let sx_val: usize = s[sx + 10..].lines().next().unwrap().parse().unwrap();
        assert!(bytes[sx_val..].starts_with(b"xref\n"), "startxref must point at xref table");
    }
}
