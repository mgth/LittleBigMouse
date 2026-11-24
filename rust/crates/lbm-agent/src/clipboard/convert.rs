//! What a clipboard format is on each system, turned into what crosses the link and back
//! (docs/multi-pc-etat.md, "Formats, par palier"). Pure, so that the conversions of both
//! systems are tested on either.

use std::io::Cursor;
use std::path::PathBuf;

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

/// The local files of a `text/uri-list` (RFC 2483: one URI a line, `#` for a comment):
/// what is not a `file:` URI of this machine is left out.
pub fn paths_from_uri_list(list: &[u8]) -> Vec<PathBuf> {
    String::from_utf8_lossy(list)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| url::Url::parse(line).ok())
        .filter(|url| url.scheme() == "file")
        .filter_map(|url| url.to_file_path().ok())
        .collect()
}

/// `paths` as a `text/uri-list`, lines ended by `\r\n` as the RFC says.
pub fn uri_list(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .filter_map(|path| url::Url::from_file_path(path).ok())
        .map(|url| format!("{url}\r\n"))
        .collect()
}

/// `paths` as GNOME's file managers copy them (`x-special/gnome-copied-files`).
pub fn gnome_copied_files(paths: &[PathBuf]) -> String {
    let uris: Vec<String> = paths
        .iter()
        .filter_map(|path| url::Url::from_file_path(path).ok())
        .map(|url| url.to_string())
        .collect();
    format!("copy\n{}", uris.join("\n"))
}

/// The size of a `DROPFILES` header, which `CF_HDROP` starts with.
const DROPFILES: usize = 20;

/// The paths of a `CF_HDROP`: a `DROPFILES` header (offset of the list, a point, two
/// flags, the last saying wide characters), then the paths, each ended by a NUL, the
/// list by another.
pub fn paths_from_dropfiles(bytes: &[u8]) -> Vec<PathBuf> {
    if bytes.len() < DROPFILES {
        return Vec::new();
    }
    let offset = u32::from_le_bytes(bytes[0..4].try_into().expect("4 bytes")) as usize;
    let wide = u32::from_le_bytes(bytes[16..20].try_into().expect("4 bytes")) != 0;
    let Some(list) = bytes.get(offset..) else {
        return Vec::new();
    };
    let names: Vec<String> = if wide {
        let units: Vec<u16> = list
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        units
            .split(|unit| *unit == 0)
            .take_while(|name| !name.is_empty())
            .map(String::from_utf16_lossy)
            .collect()
    } else {
        list.split(|byte| *byte == 0)
            .take_while(|name| !name.is_empty())
            .map(|name| String::from_utf8_lossy(name).into_owned())
            .collect()
    };
    names.into_iter().map(PathBuf::from).collect()
}

/// `paths` as a `CF_HDROP`, wide characters.
pub fn dropfiles(paths: &[PathBuf]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(DROPFILES as u32).to_le_bytes());
    bytes.extend_from_slice(&[0; 12]); // the point, and not in a non-client area
    bytes.extend_from_slice(&1u32.to_le_bytes());
    for path in paths {
        for unit in path.to_string_lossy().encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes.extend_from_slice(&[0, 0]);
    }
    bytes.extend_from_slice(&[0, 0]);
    bytes
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
    fn a_drop_list_reads_back_its_paths() {
        let paths = vec![
            PathBuf::from("C:\\Users\\me\\été.txt"),
            PathBuf::from("D:\\b"),
        ];
        assert_eq!(paths_from_dropfiles(&dropfiles(&paths)), paths);
        assert!(paths_from_dropfiles(&[1, 2]).is_empty());
        // A narrow list, as an older application may set it.
        let mut narrow = vec![0u8; 20];
        narrow[0] = 20;
        narrow.extend_from_slice(b"C:\\a\0C:\\b\0\0");
        assert_eq!(
            paths_from_dropfiles(&narrow),
            [PathBuf::from("C:\\a"), PathBuf::from("C:\\b")]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_uri_list_holds_the_local_files_and_reads_back() {
        let paths = vec![PathBuf::from("/home/me/été 1.txt"), PathBuf::from("/tmp/b")];
        let list = uri_list(&paths);
        assert_eq!(
            list,
            "file:///home/me/%C3%A9t%C3%A9%201.txt\r\nfile:///tmp/b\r\n"
        );
        assert_eq!(paths_from_uri_list(list.as_bytes()), paths);
        assert_eq!(
            paths_from_uri_list(b"# comment\nhttps://example.org/\nfile:///x\n"),
            [PathBuf::from("/x")]
        );
        assert_eq!(gnome_copied_files(&paths[1..]), "copy\nfile:///tmp/b");
    }

    #[test]
    fn what_is_not_a_picture_is_an_error_not_a_panic() {
        assert!(dib_to_png(&[1, 2, 3]).is_err());
        assert!(dib_to_png(&[0; 60]).is_err());
        assert!(png_to_dib(b"not a png").is_err());
        assert!(!is_png(b"GIF89a"));
    }
}
