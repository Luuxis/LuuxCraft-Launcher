//! GDI+ : décoder un PNG et le dessiner à l'échelle, transparence comprise.
//!
//! Livré avec Windows depuis XP et appelé par son API C « plate » : pas de
//! COM à implémenter, pas de décodeur d'image à embarquer dans un binaire qui
//! doit rester petit. Le logo du serveur arrive en mémoire (voir `logo`) et
//! y reste : `SHCreateMemStream` en fait un flux que GDI+ sait lire.
//!
//! GDI+ garde une référence sur le flux tant que l'image existe — il peut le
//! relire pour décoder à la demande — d'où les deux relâchés ensemble.

use std::ptr;

use windows_sys::Win32::Graphics::Gdi::HDC;
use windows_sys::Win32::Graphics::GdiPlus::{
    CompositingQualityHighQuality, GdipCreateBitmapFromScan0, GdipCreateFromHDC,
    GdipCreateHICONFromBitmap, GdipDeleteGraphics, GdipDisposeImage, GdipDrawImageRectI,
    GdipGetImageGraphicsContext, GdipGetImageHeight, GdipGetImageWidth, GdipGraphicsClear,
    GdipLoadImageFromStream, GdipSetCompositingQuality, GdipSetInterpolationMode,
    GdipSetPixelOffsetMode, GdiplusShutdown, GdiplusStartup, GdiplusStartupInput, GpGraphics,
    GpImage, InterpolationModeHighQualityBicubic, Ok as GDIPLUS_OK, PixelOffsetModeHighQuality,
};
use windows_sys::Win32::UI::Shell::SHCreateMemStream;
use windows_sys::Win32::UI::WindowsAndMessaging::HICON;

use super::Com;

/// `PixelFormat32bppARGB` : 32 bits, canal alpha, prémultiplication absente.
const PIXEL_FORMAT_32BPP_ARGB: i32 = 0x0026_200A;

/// GDI+ démarré pour la durée de vie de cet objet.
pub struct Runtime {
    token: usize,
}

impl Runtime {
    pub fn start() -> Option<Self> {
        let input = GdiplusStartupInput {
            GdiplusVersion: 1,
            DebugEventCallback: 0,
            SuppressBackgroundThread: 0,
            SuppressExternalCodecs: 0,
        };
        let mut token = 0usize;
        // SÛRETÉ : structures valides, sortie facultative laissée nulle.
        let status = unsafe { GdiplusStartup(&mut token, &input, ptr::null_mut()) };
        (status == GDIPLUS_OK).then_some(Self { token })
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // SÛRETÉ : fait pendant au démarrage réussi.
        unsafe { GdiplusShutdown(self.token) };
    }
}

/// Une image décodée, prête à être dessinée.
pub struct Image {
    image: *mut GpImage,
    /// Le flux source, ou nul pour une image créée en mémoire.
    stream: Option<Com>,
    width: u32,
    height: u32,
}

impl Image {
    /// Décode un PNG (ou tout format que GDI+ connaît) depuis la mémoire.
    pub fn from_png(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() || bytes.len() > u32::MAX as usize {
            return None;
        }
        // SÛRETÉ : le flux copie les octets, l'image est vérifiée non nulle
        // avant d'être interrogée, et tout est relâché en cas d'échec.
        unsafe {
            let raw = SHCreateMemStream(bytes.as_ptr(), bytes.len() as u32);
            if raw.is_null() {
                return None;
            }
            let stream = Com(raw);

            let mut image: *mut GpImage = ptr::null_mut();
            if GdipLoadImageFromStream(stream.0, &mut image) != GDIPLUS_OK || image.is_null() {
                return None;
            }

            let (mut width, mut height) = (0u32, 0u32);
            GdipGetImageWidth(image, &mut width);
            GdipGetImageHeight(image, &mut height);
            if width == 0 || height == 0 {
                GdipDisposeImage(image);
                return None;
            }

            Some(Self {
                image,
                stream: Some(stream),
                width,
                height,
            })
        }
    }

    /// Dessine l'image dans le rectangle donné, sans la déformer : mise à
    /// l'échelle pour y tenir, centrée.
    pub fn draw(&self, hdc: HDC, x: i32, y: i32, width: i32, height: i32) {
        let (dx, dy, dw, dh) = fit(self.width, self.height, x, y, width, height);
        // SÛRETÉ : `hdc` est un contexte valide fourni par `BeginPaint` ;
        // l'objet graphique est supprimé avant de rendre la main.
        unsafe {
            let mut graphics: *mut GpGraphics = ptr::null_mut();
            if GdipCreateFromHDC(hdc, &mut graphics) != GDIPLUS_OK || graphics.is_null() {
                return;
            }
            configure(graphics);
            GdipDrawImageRectI(graphics, self.image, dx, dy, dw, dh);
            GdipDeleteGraphics(graphics);
        }
    }

    /// Une icône de fenêtre carrée, de `size` pixels de côté.
    pub fn to_icon(&self, size: i32) -> HICON {
        let (dx, dy, dw, dh) = fit(self.width, self.height, 0, 0, size, size);
        // SÛRETÉ : bitmap et graphique sont vérifiés puis détruits ici ; le
        // `GpBitmap` est un `GpImage` pour GDI+, d'où la conversion.
        unsafe {
            let mut bitmap = ptr::null_mut();
            let created = GdipCreateBitmapFromScan0(
                size,
                size,
                0,
                PIXEL_FORMAT_32BPP_ARGB,
                ptr::null(),
                &mut bitmap,
            );
            if created != GDIPLUS_OK || bitmap.is_null() {
                return ptr::null_mut();
            }

            let mut graphics: *mut GpGraphics = ptr::null_mut();
            if GdipGetImageGraphicsContext(bitmap as *mut GpImage, &mut graphics) == GDIPLUS_OK
                && !graphics.is_null()
            {
                GdipGraphicsClear(graphics, 0);
                configure(graphics);
                GdipDrawImageRectI(graphics, self.image, dx, dy, dw, dh);
                GdipDeleteGraphics(graphics);
            }

            let mut icon: HICON = ptr::null_mut();
            GdipCreateHICONFromBitmap(bitmap, &mut icon);
            GdipDisposeImage(bitmap as *mut GpImage);
            icon
        }
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        // SÛRETÉ : l'image est relâchée avant le flux dont elle dépend, que
        // `Com` relâche ensuite.
        unsafe { GdipDisposeImage(self.image) };
        self.stream.take();
    }
}

/// SÛRETÉ : `graphics` doit être un objet graphique GDI+ valide.
unsafe fn configure(graphics: *mut GpGraphics) {
    GdipSetInterpolationMode(graphics, InterpolationModeHighQualityBicubic);
    GdipSetPixelOffsetMode(graphics, PixelOffsetModeHighQuality);
    GdipSetCompositingQuality(graphics, CompositingQualityHighQuality);
}

/// Rectangle de destination d'une image de `width`×`height` dans la zone
/// donnée : proportions conservées, centré.
fn fit(width: u32, height: u32, x: i32, y: i32, box_width: i32, box_height: i32) -> (i32, i32, i32, i32) {
    let scale = f64::min(
        f64::from(box_width) / f64::from(width.max(1)),
        f64::from(box_height) / f64::from(height.max(1)),
    );
    let drawn_width = ((f64::from(width) * scale).round() as i32).max(1);
    let drawn_height = ((f64::from(height) * scale).round() as i32).max(1);
    (
        x + (box_width - drawn_width) / 2,
        y + (box_height - drawn_height) / 2,
        drawn_width,
        drawn_height,
    )
}
