//! Decodes images through gdk-pixbuf, falling back to the `image` crate
//! for formats the system has no gdk-pixbuf loader for (e.g. webp).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use relm4::gtk::gdk_pixbuf::{Colorspace, Pixbuf, PixbufLoader};
use relm4::gtk::glib::Bytes;
use relm4::gtk::prelude::*;

/// Mime types the fallback decoder supports on top of the gdk-pixbuf formats.
pub const FALLBACK_MIME_TYPES: &[&str] = &["image/webp"];

/// Sorted supported images in the directory containing `current`, as absolute paths.
pub fn directory_images(current: &Path) -> Result<Vec<PathBuf>> {
    let current = std::path::absolute(current)?;
    let directory = current
        .parent()
        .ok_or(anyhow!("Image has no parent directory"))?;
    let extensions: Vec<String> = Pixbuf::formats()
        .iter()
        .flat_map(|format| format.extensions())
        .map(|extension| extension.to_string())
        .chain(std::iter::once("webp".to_owned()))
        .collect();
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|extension| {
                    extensions.iter().any(|e| e.eq_ignore_ascii_case(extension))
                })
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// Pixbuf is not Send, so background decoding hands its pixels over in this form.
#[derive(Debug)]
pub struct DecodedImage {
    bytes: Bytes,
    has_alpha: bool,
    width: i32,
    height: i32,
    rowstride: i32,
}

impl DecodedImage {
    pub fn from_file(path: &Path) -> Result<Self> {
        let mut pixbuf = pixbuf_from_file(path)?;
        // GL drivers expand RGB to RGBA on the CPU during upload, so pay that here off the UI thread
        if !pixbuf.has_alpha() {
            pixbuf = pixbuf.add_alpha(false, 0, 0, 0)?;
        }
        Ok(Self {
            bytes: pixbuf.read_pixel_bytes(),
            has_alpha: pixbuf.has_alpha(),
            width: pixbuf.width(),
            height: pixbuf.height(),
            rowstride: pixbuf.rowstride(),
        })
    }

    pub fn into_pixbuf(self) -> Pixbuf {
        Pixbuf::from_bytes(
            &self.bytes,
            Colorspace::Rgb,
            self.has_alpha,
            8,
            self.width,
            self.height,
            self.rowstride,
        )
    }
}

pub fn pixbuf_from_file(path: &Path) -> Result<Pixbuf> {
    let bytes = fs::read(path).with_context(|| format!("couldn't read file {}", path.display()))?;
    pixbuf_from_bytes(&bytes)
}

pub fn pixbuf_from_bytes(bytes: &[u8]) -> Result<Pixbuf> {
    gdk_pixbuf_from_bytes(bytes).or_else(|error| {
        // on a double failure report the gdk-pixbuf error, it covers far
        // more formats than the fallback
        let image = image::load_from_memory(bytes).map_err(|_| error)?;
        Ok(pixbuf_from_rgba_image(image.into_rgba8()))
    })
}

fn gdk_pixbuf_from_bytes(bytes: &[u8]) -> Result<Pixbuf> {
    let loader = PixbufLoader::new();
    let write_result = loader.write(bytes);
    // close even a failed loader, dropping an open one makes gdk-pixbuf warn
    let close_result = loader.close();
    write_result?;
    close_result?;
    loader
        .pixbuf()
        .ok_or(anyhow!("Conversion to Pixbuf failed"))
}

fn pixbuf_from_rgba_image(image: image::RgbaImage) -> Pixbuf {
    let (width, height) = image.dimensions();
    Pixbuf::from_bytes(
        &Bytes::from_owned(image.into_raw()),
        Colorspace::Rgb,
        true,
        8,
        width as i32,
        height as i32,
        width as i32 * 4,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // a 2x2 solid green lossless webp
    const WEBP_2X2_GREEN: &[u8] = &[
        0x52, 0x49, 0x46, 0x46, 0x1c, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38,
        0x4c, 0x0f, 0x00, 0x00, 0x00, 0x2f, 0x01, 0x40, 0x00, 0x00, 0x07, 0xd0, 0xff, 0x88, 0xfe,
        0x07, 0x22, 0xa2, 0xff, 0x01, 0x00,
    ];

    #[test]
    fn decodes_webp() {
        let pixbuf = pixbuf_from_bytes(WEBP_2X2_GREEN).unwrap();
        assert_eq!((pixbuf.width(), pixbuf.height()), (2, 2));
        let pixels = unsafe { pixbuf.pixels() };
        assert_eq!(&pixels[..3], &[0x00, 0xff, 0x00]);
    }

    #[test]
    fn decodes_gdk_pixbuf_formats() {
        let pixbuf = Pixbuf::new(Colorspace::Rgb, false, 8, 2, 2).unwrap();
        pixbuf.fill(0xff0000ff);
        let png = pixbuf.save_to_bufferv("png", &[]).unwrap();

        let decoded = pixbuf_from_bytes(&png).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (2, 2));
        let pixels = unsafe { decoded.pixels() };
        assert_eq!(&pixels[..3], &[0xff, 0x00, 0x00]);
    }

    #[test]
    fn rejects_data_that_is_not_an_image() {
        assert!(pixbuf_from_bytes(b"not an image").is_err());
    }
}
