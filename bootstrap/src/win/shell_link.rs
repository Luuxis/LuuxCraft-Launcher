//! Écriture d'un raccourci `.lnk` par le shell de Windows (`IShellLinkW`).
//!
//! Le format `.lnk` (MS-SHLLINK) a été écrit à la main dans une version
//! précédente, et le résultat ne s'ouvrait pas : il décrivait la cible par son
//! chemin (`LinkInfo`) sans la liste d'identifiants du shell
//! (`LinkTargetIDList`), qui est ce qu'Explorer suit réellement. Cette liste
//! est faite d'éléments dont la forme dépend de la version de Windows ; le
//! seul code qui la produit à coup sûr est celui qui la relit. On le lui
//! demande donc, par COM, en processus : aucun `powershell.exe` — que la
//! moitié des antivirus grand public bloquent — et rien de plus dans le
//! binaire que trois tables de fonctions.
//!
//! Un échec n'est jamais fatal (voir `shortcuts`) : sans raccourci, le client
//! est installé et lancé quand même.

use std::ffi::c_void;
use std::fs;
use std::io;
use std::path::Path;
use std::ptr;

use windows_sys::core::{BOOL, GUID, HRESULT, PCWSTR};
use windows_sys::Win32::Foundation::{RPC_E_CHANGED_MODE, S_FALSE, S_OK};
use windows_sys::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use super::{check, hresult_error, wide, wide_os, Com, IUnknownVtbl};

const CLSID_SHELL_LINK: GUID = GUID::from_u128(0x00021401_0000_0000_c000_000000000046);
const IID_ISHELL_LINK_W: GUID = GUID::from_u128(0x000214f9_0000_0000_c000_000000000046);
const IID_IPERSIST_FILE: GUID = GUID::from_u128(0x0000010b_0000_0000_c000_000000000046);

/// `IShellLinkW`, dans l'ordre exact de `shobjidl_core.h`. Les fonctions qui
/// ne sont pas appelées n'ont besoin que d'occuper leur place.
#[repr(C)]
struct IShellLinkWVtbl {
    base: IUnknownVtbl,
    get_path: usize,
    get_id_list: usize,
    set_id_list: usize,
    get_description: usize,
    set_description: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    get_working_directory: usize,
    set_working_directory: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
    get_arguments: usize,
    set_arguments: usize,
    get_hotkey: usize,
    set_hotkey: usize,
    get_show_cmd: usize,
    set_show_cmd: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
    get_icon_location: usize,
    set_icon_location: unsafe extern "system" fn(*mut c_void, PCWSTR, i32) -> HRESULT,
    set_relative_path: usize,
    resolve: usize,
    set_path: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT,
}

/// `IPersistFile` (`objidl.h`), qui hérite d'`IPersist` : `GetClassID` vient
/// avant ses propres fonctions.
#[repr(C)]
struct IPersistFileVtbl {
    base: IUnknownVtbl,
    get_class_id: usize,
    is_dirty: usize,
    load: usize,
    save: unsafe extern "system" fn(*mut c_void, PCWSTR, BOOL) -> HRESULT,
    save_completed: usize,
    get_cur_file: usize,
}

/// Initialisation de COM sur le fil courant, défaite en sortie de portée.
///
/// `RPC_E_CHANGED_MODE` veut dire que COM est déjà initialisé ici dans un
/// autre mode : ce n'est pas une erreur, l'objet se crée quand même, et ce
/// n'est pas à nous de le désinitialiser.
struct Apartment {
    uninitialize: bool,
}

impl Apartment {
    fn enter() -> io::Result<Self> {
        // SÛRETÉ : appel sans précondition ; `CoUninitialize` n'est fait que
        // si cet appel-ci a compté.
        let result = unsafe {
            CoInitializeEx(
                ptr::null(),
                (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
            )
        };
        match result {
            S_OK | S_FALSE => Ok(Self { uninitialize: true }),
            RPC_E_CHANGED_MODE => Ok(Self {
                uninitialize: false,
            }),
            other => Err(hresult_error("CoInitializeEx", other)),
        }
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        if self.uninitialize {
            // SÛRETÉ : fait pendant à l'initialisation réussie ci-dessus.
            unsafe { CoUninitialize() };
        }
    }
}

/// Écrit `lnk`, pointant sur `target` depuis `working_dir`, avec l'icône et la
/// description données.
pub fn write(
    lnk: &Path,
    target: &Path,
    working_dir: &Path,
    icon: Option<&Path>,
    description: &str,
) -> io::Result<()> {
    if let Some(parent) = lnk.parent() {
        fs::create_dir_all(parent)?;
    }
    let _apartment = Apartment::enter()?;

    // SÛRETÉ : chaque pointeur d'interface est vérifié non nul avant usage,
    // et relâché par `Com` ; les chaînes passées vivent le temps de l'appel.
    unsafe {
        let mut raw: *mut c_void = ptr::null_mut();
        check(
            "CoCreateInstance(ShellLink)",
            CoCreateInstance(
                &CLSID_SHELL_LINK,
                ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_ISHELL_LINK_W,
                &mut raw,
            ),
        )?;
        if raw.is_null() {
            return Err(io::Error::other("CoCreateInstance(ShellLink) : objet nul"));
        }
        let link = Com(raw);
        let vtbl: &IShellLinkWVtbl = link.vtbl();

        check("SetPath", (vtbl.set_path)(link.0, wide_os(target.as_os_str()).as_ptr()))?;
        check(
            "SetWorkingDirectory",
            (vtbl.set_working_directory)(link.0, wide_os(working_dir.as_os_str()).as_ptr()),
        )?;
        check(
            "SetDescription",
            (vtbl.set_description)(link.0, wide(description).as_ptr()),
        )?;
        check("SetShowCmd", (vtbl.set_show_cmd)(link.0, SW_SHOWNORMAL))?;
        if let Some(icon) = icon {
            check(
                "SetIconLocation",
                (vtbl.set_icon_location)(link.0, wide_os(icon.as_os_str()).as_ptr(), 0),
            )?;
        }

        let mut raw_file: *mut c_void = ptr::null_mut();
        check(
            "QueryInterface(IPersistFile)",
            (vtbl.base.query_interface)(link.0, &IID_IPERSIST_FILE, &mut raw_file),
        )?;
        if raw_file.is_null() {
            return Err(io::Error::other("QueryInterface(IPersistFile) : objet nul"));
        }
        let file = Com(raw_file);
        let file_vtbl: &IPersistFileVtbl = file.vtbl();
        check(
            "Save",
            (file_vtbl.save)(file.0, wide_os(lnk.as_os_str()).as_ptr(), 1),
        )?;
    }

    Ok(())
}
