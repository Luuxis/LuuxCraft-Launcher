//! Icône de l'exécutable, tirée de `logo/logo.png`.
//!
//! Windows attend une icône à plusieurs tailles : 16 et 32 px pour la barre
//! des tâches et les listes de l'Explorateur, 48 et 64 pour les vues en
//! vignettes, 256 pour les très grandes icônes et le zoom. Il choisit la plus
//! proche et met à l'échelle le reste.
//!
//! Les petites tailles sont des bitmaps DIB, le format historique que tout
//! lit. La 256 est un PNG : autorisé depuis Vista, et sept fois plus léger
//! qu'un DIB de 256 Ko dans un binaire qui doit arriver en quelques secondes.

use image::imageops::FilterType;
use image::{ImageEncoder, RgbaImage};

/// Tailles produites, la plus grande en dernier.
pub const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

/// En dessous, un DIB ; à partir de là, un PNG.
const PNG_FROM: u32 = 256;

/// En-tête `ICONDIR`, puis une `ICONDIRENTRY` par image.
const ICONDIR_SIZE: usize = 6;
const ICONDIRENTRY_SIZE: usize = 16;

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

pub struct Frame {
    pub size: u32,
    /// Un DIB (`BITMAPINFOHEADER`, pixels XOR, masque AND) ou un fichier PNG
    /// complet : exactement ce qu'une entrée d'icône contient.
    pub data: Vec<u8>,
}

impl Frame {
    pub fn is_png(&self) -> bool {
        self.data.starts_with(&PNG_SIGNATURE)
    }
}

/// Décode le PNG source et le décline dans toutes les tailles.
pub fn frames(png: &[u8]) -> Result<Vec<Frame>, String> {
    let source = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|error| format!("PNG illisible ({error})"))?
        .to_rgba8();
    let square = squared(source);

    SIZES
        .iter()
        .map(|&size| {
            let scaled = image::imageops::resize(&square, size, size, FilterType::Lanczos3);
            let data = if size >= PNG_FROM {
                encode_png(&scaled)?
            } else {
                dib(&scaled)
            };
            Ok(Frame { size, data })
        })
        .collect()
}

/// Un logo rectangulaire est centré sur un carré transparent : une icône est
/// carrée, et l'étirer déformerait le logo.
fn squared(image: RgbaImage) -> RgbaImage {
    let (width, height) = image.dimensions();
    if width == height {
        return image;
    }
    let side = width.max(height);
    let mut canvas = RgbaImage::from_pixel(side, side, image::Rgba([0, 0, 0, 0]));
    let x = i64::from((side - width) / 2);
    let y = i64::from((side - height) / 2);
    image::imageops::overlay(&mut canvas, &image, x, y);
    canvas
}

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new_with_quality(
        &mut out,
        image::codecs::png::CompressionType::Best,
        image::codecs::png::FilterType::Adaptive,
    );
    encoder
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|error| format!("encodage PNG impossible ({error})"))?;
    Ok(out)
}

/// Bitmap d'icône : `BITMAPINFOHEADER` (hauteur doublée, comme l'exige le
/// format), pixels BGRA de bas en haut, puis le masque AND à 1 bit par pixel
/// où 1 signifie transparent.
fn dib(image: &RgbaImage) -> Vec<u8> {
    let (width, height) = image.dimensions();
    let mask_stride = width.div_ceil(32) * 4;
    let xor_size = width * height * 4;
    let and_size = mask_stride * height;

    let mut out = Vec::with_capacity(40 + (xor_size + and_size) as usize);
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&((height * 2) as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&(xor_size + and_size).to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    out.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    out.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    out.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

    for y in (0..height).rev() {
        for x in 0..width {
            let pixel = image.get_pixel(x, y).0;
            out.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }

    for y in (0..height).rev() {
        let mut row = vec![0u8; mask_stride as usize];
        for x in 0..width {
            if image.get_pixel(x, y).0[3] < 128 {
                row[(x / 8) as usize] |= 0x80 >> (x % 8);
            }
        }
        out.extend_from_slice(&row);
    }

    out
}

/// Octet de dimension d'une entrée : 0 veut dire 256.
fn dimension_byte(size: u32) -> u8 {
    if size >= 256 {
        0
    } else {
        size as u8
    }
}

/// Fichier `.ico` complet.
pub fn file(frames: &[Frame]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // réservé
    out.extend_from_slice(&1u16.to_le_bytes()); // type : icône
    out.extend_from_slice(&(frames.len() as u16).to_le_bytes());

    let mut offset = ICONDIR_SIZE + frames.len() * ICONDIRENTRY_SIZE;
    for frame in frames {
        out.push(dimension_byte(frame.size));
        out.push(dimension_byte(frame.size));
        out.push(0); // palette : sans objet en 32 bits
        out.push(0); // réservé
        out.extend_from_slice(&1u16.to_le_bytes()); // plans
        out.extend_from_slice(&32u16.to_le_bytes()); // bits par pixel
        out.extend_from_slice(&(frame.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += frame.data.len();
    }
    for frame in frames {
        out.extend_from_slice(&frame.data);
    }
    out
}

/// Ressource `RT_GROUP_ICON` : le même répertoire, sauf que chaque entrée
/// désigne une ressource `RT_ICON` par son identifiant au lieu d'un décalage
/// dans le fichier.
pub fn group(frames: &[Frame], first_id: u16) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(frames.len() as u16).to_le_bytes());
    for (index, frame) in frames.iter().enumerate() {
        out.push(dimension_byte(frame.size));
        out.push(dimension_byte(frame.size));
        out.push(0);
        out.push(0);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(frame.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(first_id + index as u16).to_le_bytes());
    }
    out
}
