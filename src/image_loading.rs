//! Decodes images through gdk-pixbuf, falling back to the `image` crate
//! for formats the system has no gdk-pixbuf loader for (e.g. webp).

use crate::START_TIME;
use crate::configuration::{APP_CONFIG, Input};
use crate::logging::{LogLevel, log};
use crate::{generate_profile_output, image_loading};
use anyhow::{Context, Result, anyhow};
use relm4::gtk::gdk_pixbuf::{Colorspace, Pixbuf, PixbufLoader};
use relm4::gtk::glib::Bytes;
use relm4::gtk::prelude::*;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{fs, io};

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
    sort_as_launched(&mut files);
    Ok(files)
}

/// Orders `files` as the file manager that opened the image shows its folder: FILEY_SORT is "name", "type", "modified" or "size", with
/// ",desc" after it the other way round, as Filey passes it; by name without it. Ties go by name.
fn sort_as_launched(files: &mut [PathBuf]) {
    let order = std::env::var("FILEY_SORT").unwrap_or_default();
    let (key, descending) = order
        .split_once(',')
        .map_or((order.as_str(), false), |(key, way)| (key, way == "desc"));
    let name = |path: &PathBuf| {
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let extension = |path: &PathBuf| {
        path.extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    };
    let metadata = |path: &PathBuf| fs::metadata(path).ok();
    let mut keyed: Vec<(PathBuf, String, String, u64, std::time::SystemTime)> = files
        .iter()
        .map(|path| {
            let meta = metadata(path);
            let size = meta.as_ref().map_or(0, |m| m.len());
            let modified = meta
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::UNIX_EPOCH);
            (path.clone(), name(path), extension(path), size, modified)
        })
        .collect();
    keyed.sort_by(|a, b| {
        let by = match key {
            "type" => a.2.cmp(&b.2),
            "modified" => a.4.cmp(&b.4),
            "size" => a.3.cmp(&b.3),
            _ => std::cmp::Ordering::Equal,
        };
        let by = if descending { by.reverse() } else { by };
        let names = natural_order(&a.1, &b.1);
        by.then(
            if descending && key != "type" && key != "modified" && key != "size" {
                names.reverse()
            } else {
                names
            },
        )
    });
    for (slot, (path, ..)) in files.iter_mut().zip(keyed) {
        *slot = path;
    }
}

/// Names as Filey orders them: ASCII letters without regard to case, and runs of digits by their value.
fn natural_order(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let (mut ie, mut je) = (i, j);
            while ie < a.len() && a[ie].is_ascii_digit() {
                ie += 1;
            }
            while je < b.len() && b[je].is_ascii_digit() {
                je += 1;
            }
            let (mut iz, mut jz) = (i, j);
            while iz < ie && a[iz] == b'0' {
                iz += 1;
            }
            while jz < je && b[jz] == b'0' {
                jz += 1;
            }
            let by = (ie - iz)
                .cmp(&(je - jz))
                .then_with(|| a[iz..ie].cmp(&b[jz..je]));
            if by != Ordering::Equal {
                return by;
            }
            i = ie;
            j = je;
        } else {
            let by = a[i].to_ascii_lowercase().cmp(&b[j].to_ascii_lowercase());
            if by != Ordering::Equal {
                return by;
            }
            i += 1;
            j += 1;
        }
    }
    (a.len() - i).cmp(&(b.len() - j)).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod order_tests {
    use super::natural_order;
    use std::cmp::Ordering;

    #[test]
    fn names_order_as_filey_does() {
        assert_eq!(natural_order("img2.png", "img10.png"), Ordering::Less);
        assert_eq!(natural_order("B.png", "a.png"), Ordering::Greater);
        assert_eq!(natural_order("a01.png", "a1.png"), "a01.png".cmp("a1.png"));
    }
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

pub(crate) fn load_image() -> Result<Pixbuf> {
    // load app config
    let config = APP_CONFIG.read();

    generate_profile_output!("loading image");
    // load input image
    let image = match config.input() {
        Some(Input::Stdin) => {
            let mut buf = Vec::<u8>::new();
            io::stdin().lock().read_to_end(&mut buf)?;
            image_loading::pixbuf_from_bytes(&buf).context("couldn't load image from stdin")?
        }
        Some(Input::ClipboardCommand(paste_command)) => {
            let mut buf = Vec::<u8>::new();
            let mut child = Command::new("sh")
                .arg("-c")
                .arg(paste_command)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .spawn()?;

            child.stdout.take().unwrap().read_to_end(&mut buf)?;
            image_loading::pixbuf_from_bytes(&buf)
                .context(format!("couldn't load image from {:?}", paste_command))?
        }
        Some(Input::File(input_filename)) => {
            image_loading::pixbuf_from_file(Path::new(input_filename))
                .context("couldn't load image")?
        }
        None => {
            anyhow::bail!("no input provided");
        }
    };
    generate_profile_output!("image loaded");
    Ok(image)
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
