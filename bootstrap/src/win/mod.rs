//! Ce qui n'existe que sous Windows : la fenêtre, GDI+, le shell.
//!
//! Tout passe par `windows-sys`, qui n'est que des déclarations : seules les
//! fonctions appelées entrent dans le binaire. Les interfaces COM dont on a
//! besoin (`IShellLinkW`, `IPersistFile`, et `Release` sur un flux) sont
//! décrites ici par leur table de fonctions, ce qui évite d'embarquer une
//! bibliothèque COM entière pour trois appels.

pub mod gdiplus;
pub mod gui;
pub mod shell_link;

use std::ffi::{c_void, OsStr, OsString};
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::ptr;

use windows_sys::core::{GUID, HRESULT, PWSTR};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;

/// Chaîne UTF-16 terminée par zéro, pour les fonctions `…W`.
pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Idem pour un chemin, sans passer par UTF-8 : un dossier utilisateur peut
/// porter des unités UTF-16 qui n'y sont pas représentables.
pub fn wide_os(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain(std::iter::once(0)).collect()
}

/// Un dossier connu du shell (Bureau, menu Démarrer…), là où Windows le met
/// pour cet utilisateur — y compris redirigé, sur OneDrive par exemple.
pub fn known_folder(id: &GUID) -> Option<PathBuf> {
    let mut path: PWSTR = ptr::null_mut();
    // SÛRETÉ : `path` reçoit un tampon alloué par le shell, libéré ci-dessous
    // par `CoTaskMemFree` comme la documentation l'exige.
    unsafe {
        let result = SHGetKnownFolderPath(id, 0, ptr::null_mut(), &mut path);
        if result < 0 || path.is_null() {
            return None;
        }
        let length = (0..).take_while(|&index| *path.add(index) != 0).count();
        let folder = OsString::from_wide(std::slice::from_raw_parts(path, length));
        CoTaskMemFree(path as *const c_void);
        Some(PathBuf::from(folder))
    }
}

/// Les trois fonctions que toute interface COM commence par déclarer.
#[repr(C)]
pub struct IUnknownVtbl {
    pub query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    pub release: unsafe extern "system" fn(*mut c_void) -> u32,
}

/// Un objet COM, relâché quand il sort de portée.
pub struct Com(pub *mut c_void);

impl Com {
    /// SÛRETÉ : `object` doit être un pointeur d'interface COM valide dont
    /// l'appelant cède la référence.
    pub unsafe fn vtbl<T>(&self) -> &T {
        &**(self.0 as *mut *const T)
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SÛRETÉ : toute interface COM commence par la table d'`IUnknown`.
            unsafe {
                let vtbl: &IUnknownVtbl = self.vtbl();
                (vtbl.release)(self.0);
            }
        }
    }
}

/// Une erreur COM, avec le code que le joueur pourra rapporter.
pub fn hresult_error(what: &str, result: HRESULT) -> io::Error {
    io::Error::other(format!("{what} : HRESULT 0x{:08X}", result as u32))
}

/// Vérifie un `HRESULT`.
pub fn check(what: &str, result: HRESULT) -> io::Result<()> {
    if result < 0 {
        Err(hresult_error(what, result))
    } else {
        Ok(())
    }
}
