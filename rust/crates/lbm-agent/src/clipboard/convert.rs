//! What a clipboard format is on each system, turned into what crosses the link and back
//! (docs/multi-pc-etat.md, "Formats, par palier"). Pure, so that the conversions of both
//! systems are tested on either.

use std::io::Cursor;

/// Text as it crosses: lines ended by `\n`, whatever ended them here.
pub fn text_to_wire(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Text crossing to Windows, whose applications expect `\r\n`. A `\r\n` already there
/// stays one.
pub fn text_to_windows(text: &str) -> String {
    text_to_wire(text).replace('\n', "\r\n")
}

/// A DIB, as Windows holds `CF_DIB` (a `BITMAPINFOHEADER` or a later version, the
/// colour table or masks, the pixels, no file header), as a PNG.
///
/// A 32-bit `BI_RGB` DIB has an alpha byte that nothing promises is meaningful (a
/// screenshot often leaves it at zero): it is read as opaque. Where transparency matters,
/// Windows applications offer the registered `PNG` format too, which is read instead.
pub fn dib_to_png(dib: &[u8]) -> Result<Vec<u8>, String> {
    let field = |at: usize, len: usize| -> Result<&[u8], String> {
        dib.get(at..at + len)
            .ok_or_else(|| "the picture is cut short".to_owned())
    };
    let u32_at = |at: usize| -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            field(at, 4)?.try_into().expect("4 bytes"),
        ))
    };
    let header = u32_at(0)? as usize;
    if header < 40 {
        return Err("the picture has a header this does not read".to_owned());
    }
    let bit_count = u16::from_le_bytes(field(14, 2)?.try_into().expect("2 bytes"));
    let compression = u32_at(16)?;
    let colors_used = u32_at(32)? as usize;
    const BI_BITFIELDS: u32 = 3;
    // The masks follow a bare BITMAPINFOHEADER; later headers hold them.
    let masks = if compression == BI_BITFIELDS && header == 40 {
        12
    } else {
        0
    };
    let palette = if bit_count <= 8 {
        4 * if colors_used == 0 {
            1usize << bit_count
        } else {
            colors_used
        }
    } else {
        4 * colors_used
    };
    let offset = 14 + header + masks + palette;
    let mut file = Vec::with_capacity(14 + dib.len());
    file.extend_from_slice(b"BM");
    file.extend_from_slice(&((14 + dib.len()) as u32).to_le_bytes());
    file.extend_from_slice(&[0; 4]);
    file.extend_from_slice(&(offset as u32).to_le_bytes());
    file.extend_from_slice(dib);
    let picture = image::load_from_memory_with_format(&file, image::ImageFormat::Bmp)
        .map_err(|error| format!("the picture cannot be read: {error}"))?;
    png(&picture)
}

/// A PNG as a DIB for `CF_DIB`: 32 bits a pixel, bottom-up, `BI_RGB`, the alpha in the
/// fourth byte for the applications that read it.
pub fn png_to_dib(png: &[u8]) -> Result<Vec<u8>, String> {
    let picture = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|error| format!("the picture cannot be read: {error}"))?
        .to_rgba8();
    let (width, height) = picture.dimensions();
    let size = width as usize * height as usize * 4;
    let mut dib = Vec::with_capacity(40 + size);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&32u16.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    dib.extend_from_slice(&(size as u32).to_le_bytes());
    dib.extend_from_slice(&[0; 16]); // resolution, colours used and important
    for row in picture.rows().rev() {
        for pixel in row {
            let [r, g, b, a] = pixel.0;
            dib.extend_from_slice(&[b, g, r, a]);
        }
    }
    Ok(dib)
}

/// Whether `bytes` is a PNG: what crosses as one is checked before it is offered on as
/// one.
pub fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
}

fn png(picture: &image::DynamicImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    picture
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .map_err(|error| format!("the picture cannot be written: {error}"))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn line_ends_cross_as_newlines_and_reach_windows_as_crlf() {
        assert_eq!(text_to_wire("a\r\nb\nc"), "a\nb\nc");
        assert_eq!(text_to_windows("a\nb\r\nc"), "a\r\nb\r\nc");
        assert_eq!(text_to_windows("é\n"), "é\r\n");
    }

    fn sample() -> RgbaImage {
        RgbaImage::from_fn(3, 2, |x, y| Rgba([x as u8 * 80, y as u8 * 120, 7, 255]))
    }

    #[test]
    fn a_png_reaches_windows_as_a_bottom_up_dib_and_comes_back_the_same() {
        let png = png(&image::DynamicImage::ImageRgba8(sample())).unwrap();
        assert!(is_png(&png));
        let dib = png_to_dib(&png).unwrap();
        assert_eq!(dib.len(), 40 + 3 * 2 * 4);
        // The first row of a bottom-up DIB is the picture's last: (0,1) in BGRA.
        assert_eq!(&dib[40..44], &[7, 120, 0, 255]);
        let back = dib_to_png(&dib).unwrap();
        let back = image::load_from_memory(&back).unwrap().to_rgba8();
        assert_eq!(back, sample());
    }

    #[test]
    fn a_24_bit_dib_from_windows_is_read_with_its_row_padding() {
        // 2×2, 24 bits, rows padded to 8 bytes, bottom-up.
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&2i32.to_le_bytes());
        dib.extend_from_slice(&2i32.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&24u16.to_le_bytes());
        dib.extend_from_slice(&[0; 24]);
        dib.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0]); // bottom: blue, green
        dib.extend_from_slice(&[0, 0, 255, 255, 255, 255, 0, 0]); // top: red, white
        let png = dib_to_png(&dib).unwrap();
        let read = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(read.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(read.get_pixel(1, 0).0, [255, 255, 255, 255]);
        assert_eq!(read.get_pixel(0, 1).0, [0, 0, 255, 255]);
        assert_eq!(read.get_pixel(1, 1).0, [0, 255, 0, 255]);
    }

    #[test]
    fn what_is_not_a_picture_is_an_error_not_a_panic() {
        assert!(dib_to_png(&[1, 2, 3]).is_err());
        assert!(dib_to_png(&[0; 60]).is_err());
        assert!(png_to_dib(b"not a png").is_err());
        assert!(!is_png(b"GIF89a"));
    }
}
