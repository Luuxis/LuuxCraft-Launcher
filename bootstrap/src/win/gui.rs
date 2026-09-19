//! La fenêtre d'installation.
//!
//! Du Win32 natif : une fenêtre, des contrôles standard, GDI+ pour le logo.
//! Ni tauri, ni webview, ni bibliothèque d'interface — c'est le premier
//! fichier que le joueur télécharge, il doit rester minuscule et s'ouvrir
//! immédiatement.
//!
//! Ce que le joueur voit, dans l'ordre :
//!
//! 1. la fenêtre s'ouvre tout de suite, avec le logo générique, pendant que
//!    le panel est interrogé ;
//! 2. le nom et le logo du serveur remplacent le générique, le dossier
//!    d'installation est annoncé, et le joueur choisit s'il veut un raccourci
//!    sur le Bureau — coché par défaut ; le menu Démarrer en reçoit toujours
//!    un ;
//! 3. « Installer » lance le travail sur un fil séparé, qui rapporte sa
//!    progression par messages : la fenêtre ne se fige jamais ;
//! 4. le moteur démarre et la fenêtre se ferme. En cas d'échec, l'erreur est
//!    affichée avec son conseil, et « Réessayer » recommence depuis le début.
//!
//! ## Deux fils, un contrat
//!
//! Le fil de travail n'appelle jamais une fonction de fenêtre autre que
//! `PostMessageW`, faite pour ça. Il écrit ce qu'il a à dire dans un
//! instantané partagé (`Snapshot`), puis prévient la fenêtre, qui relit
//! l'instantané sur son propre fil. Aucun contrôle n'est jamais touché
//! depuis deux fils.
//!
//! ## Réentrance
//!
//! Modifier un contrôle (`SetWindowTextW`, `EnableWindow`…) peut le faire
//! redessiner **immédiatement**, donc renvoyer un message à cette procédure
//! de fenêtre avant que l'appel ne rende la main. L'état mutable n'est donc
//! jamais emprunté pendant un appel Win32 : on lit ce qu'il faut, on relâche,
//! puis on appelle. Les poignées, elles, sont copiables et lues sans emprunt.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::mem;
use std::ptr;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use windows_sys::core::{w, PCWSTR};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontIndirectW, EndPaint, GetDC, GetDeviceCaps, GetSysColorBrush,
    InvalidateRect, ReleaseDC, SetBkMode, SetTextColor, UpdateWindow, CLEARTYPE_QUALITY,
    COLOR_WINDOW, DEFAULT_CHARSET, FW_NORMAL, FW_SEMIBOLD, HDC, HFONT, LOGFONTW, LOGPIXELSX,
    PAINTSTRUCT, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemServices::{
    SS_EDITCONTROL, SS_ENDELLIPSIS, SS_LEFT, SS_NOPREFIX, SS_PATHELLIPSIS,
};
use windows_sys::Win32::UI::Controls::{
    InitCommonControlsEx, BST_CHECKED, ICC_PROGRESS_CLASS, ICC_STANDARD_CLASSES,
    INITCOMMONCONTROLSEX, PBM_SETMARQUEE, PBM_SETPOS, PBM_SETRANGE32, PBS_MARQUEE,
    PROGRESS_CLASSW,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetMessageW, GetSystemMetrics, IsDialogMessageW, LoadCursorW, LoadIconW, LoadImageW,
    MessageBoxW, PostMessageW, PostQuitMessage, RegisterClassExW, SendMessageW, SetWindowTextW,
    ShowWindow, SystemParametersInfoW, TranslateMessage, BM_GETCHECK, BM_SETCHECK, BN_CLICKED,
    BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON, DC_HASDEFID, DM_GETDEFID, HICON, HMENU,
    ICON_BIG, ICON_SMALL, IDCANCEL, IDC_ARROW, IMAGE_ICON, LR_DEFAULTCOLOR, MB_ICONERROR, MB_OK,
    MSG, NONCLIENTMETRICSW, SM_CXSCREEN, SM_CYSCREEN, SPI_GETNONCLIENTMETRICS, SW_HIDE, SW_SHOW,
    WM_APP, WM_CLOSE, WM_COMMAND, WM_CTLCOLORSTATIC, WM_DESTROY, WM_PAINT, WM_SETFONT,
    WM_SETICON, WNDCLASSEXW, WS_CAPTION, WS_CHILD, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
    WS_TABSTOP, WS_VISIBLE,
};

use super::gdiplus::{self, Image};
use super::wide;
use crate::error::{BootstrapError, Result};
use crate::install::{self, Options, Prepared};
use crate::logo;
use crate::net::Http;
use crate::report::{self, Report, STEPS};
use crate::resources;

const CLASS_NAME: PCWSTR = w!("LuuxCraftBootstrapWindow");
const WINDOW_TITLE: PCWSTR = w!("Installation du launcher");

/// Identifiant de l'icône de l'exécutable dans ses ressources (`build.rs`).
const ICON_RESOURCE: PCWSTR = 1 as PCWSTR;

// Identifiants des contrôles. Le bouton de fermeture porte `IDCANCEL` pour
// qu'Échap, que `IsDialogMessageW` traduit en `IDCANCEL`, fasse la même chose
// qu'un clic dessus.
const ID_TITLE: i32 = 101;
const ID_INTRO: i32 = 102;
const ID_PATH: i32 = 103;
const ID_DESKTOP: i32 = 104;
const ID_NOTE: i32 = 105;
const ID_MARQUEE: i32 = 106;
const ID_PROGRESS: i32 = 107;
const ID_STATUS: i32 = 108;
const ID_INSTALL: i32 = 109;
const ID_CANCEL: i32 = IDCANCEL;

// Messages du fil de travail vers la fenêtre.
const WM_APP_REFRESH: u32 = WM_APP + 1;
const WM_APP_PREPARED: u32 = WM_APP + 2;
const WM_APP_INSTALLED: u32 = WM_APP + 3;

/// Résolution de la barre de progression.
const PROGRESS_RANGE: i32 = 1000;

// Géométrie, en pixels logiques à 96 ppp ; `scale` l'adapte à l'écran.
const WINDOW_WIDTH: i32 = 460;
const WINDOW_HEIGHT: i32 = 356;
const MARGIN: i32 = 28;
const LOGO_SIZE: i32 = 96;
const TEXT_X: i32 = MARGIN + LOGO_SIZE + 24;
const TEXT_WIDTH: i32 = WINDOW_WIDTH - TEXT_X - MARGIN;
const FULL_WIDTH: i32 = WINDOW_WIDTH - 2 * MARGIN;
const BUTTON_HEIGHT: i32 = 28;
const BUTTON_Y: i32 = WINDOW_HEIGHT - MARGIN - BUTTON_HEIGHT;

// Couleurs, au format `COLORREF` (0x00BBGGRR).
const GREY: u32 = 0x006E_6E6E;
const RED: u32 = 0x0023_2BB5;

/// Poignées des contrôles : copiables, lues sans emprunt.
#[derive(Clone, Copy)]
struct Handles {
    window: HWND,
    title: HWND,
    intro: HWND,
    path: HWND,
    desktop: HWND,
    note: HWND,
    marquee: HWND,
    progress: HWND,
    status: HWND,
    install: HWND,
    cancel: HWND,
    dpi: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Preparing,
    Ready,
    Installing,
    Failed,
}

/// Ce que le fil de travail a à dire à la fenêtre.
#[derive(Default)]
struct Snapshot {
    step: String,
    detail: String,
    transfer: Option<Transfer>,
    outcome: Option<Outcome>,
}

#[derive(Clone)]
struct Transfer {
    label: String,
    done: u64,
    total: u64,
}

enum Outcome {
    Prepared(Result<(Prepared, Option<Vec<u8>>)>),
    Installed(Result<()>),
}

struct State {
    stage: Stage,
    prepared: Option<Arc<Prepared>>,
    logo: Option<Image>,
    shared: Arc<Mutex<Snapshot>>,
    last_status: String,
    launched: bool,
}

thread_local! {
    static HANDLES: Cell<Option<Handles>> = const { Cell::new(None) };
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Accès à l'état mutable. **Aucun appel Win32 à l'intérieur** (voir
/// l'en-tête du module).
fn with_state<R>(action: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|state| action(state.borrow_mut().as_mut().expect("état de la fenêtre")))
}

/// L'étape en cours, lisible même pendant un emprunt de l'état : les
/// messages de couleur arrivent au milieu d'un `SetWindowTextW`.
fn stage() -> Stage {
    STATE.with(|state| {
        state
            .try_borrow()
            .ok()
            .and_then(|state| state.as_ref().map(|state| state.stage))
            .unwrap_or(Stage::Preparing)
    })
}

fn lock(shared: &Mutex<Snapshot>) -> std::sync::MutexGuard<'_, Snapshot> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Ouvre la fenêtre, installe, lance ; rend le code de sortie du processus.
pub fn run() -> i32 {
    // SÛRETÉ : suite d'appels Win32 sur le fil principal, chaque poignée
    // vérifiée avant usage.
    unsafe {
        let runtime = gdiplus::Runtime::start();
        InitCommonControlsEx(&INITCOMMONCONTROLSEX {
            dwSize: mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_PROGRESS_CLASS | ICC_STANDARD_CLASSES,
        });

        let dpi = system_dpi();
        let fonts = Fonts::create(dpi);
        let Some(handles) = create_window(dpi, &fonts) else {
            MessageBoxW(
                ptr::null_mut(),
                w!("La fenêtre d'installation n'a pas pu être créée."),
                WINDOW_TITLE,
                MB_OK | MB_ICONERROR,
            );
            return 1;
        };

        let logo = runtime
            .as_ref()
            .and_then(|_| Image::from_png(resources::LOGO_PNG));
        let shared = Arc::new(Mutex::new(Snapshot::default()));
        STATE.with(|state| {
            *state.borrow_mut() = Some(State {
                stage: Stage::Preparing,
                prepared: None,
                logo,
                shared,
                last_status: String::new(),
                launched: false,
            })
        });
        HANDLES.with(|slot| slot.set(Some(handles)));

        ShowWindow(handles.window, SW_SHOW);
        UpdateWindow(handles.window);
        start_prepare(handles);

        let mut message = MSG::default();
        while GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0 {
            if IsDialogMessageW(handles.window, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

        let launched = with_state(|state| state.launched);
        // Les images doivent disparaître avant que GDI+ ne s'arrête.
        STATE.with(|state| *state.borrow_mut() = None);
        drop(runtime);
        if launched {
            0
        } else {
            1
        }
    }
}

// ------------------------------------------------------------ construction

struct Fonts {
    text: HFONT,
    title: HFONT,
}

impl Fonts {
    /// La police de message du système (Segoe UI, 9 pt), et la même en plus
    /// grand et semi-gras pour le nom du serveur.
    unsafe fn create(dpi: i32) -> Self {
        let mut base = message_font().unwrap_or_else(|| fallback_font(dpi));
        let text = CreateFontIndirectW(&base);
        base.lfHeight = -points(15, dpi);
        base.lfWeight = FW_SEMIBOLD as i32;
        let title = CreateFontIndirectW(&base);
        Self { text, title }
    }
}

unsafe fn message_font() -> Option<LOGFONTW> {
    let mut metrics: NONCLIENTMETRICSW = mem::zeroed();
    metrics.cbSize = mem::size_of::<NONCLIENTMETRICSW>() as u32;
    let read = SystemParametersInfoW(
        SPI_GETNONCLIENTMETRICS,
        metrics.cbSize,
        &mut metrics as *mut NONCLIENTMETRICSW as *mut c_void,
        0,
    );
    (read != 0).then_some(metrics.lfMessageFont)
}

fn fallback_font(dpi: i32) -> LOGFONTW {
    let mut font = LOGFONTW::default();
    font.lfHeight = -points(9, dpi);
    font.lfWeight = FW_NORMAL as i32;
    font.lfCharSet = DEFAULT_CHARSET;
    font.lfQuality = CLEARTYPE_QUALITY;
    for (slot, unit) in font.lfFaceName.iter_mut().zip("Segoe UI".encode_utf16()) {
        *slot = unit;
    }
    font
}

/// Résolution de l'écran principal ; le manifeste déclare le processus
/// conscient de la densité du système, donc rien n'est mis à l'échelle à
/// notre insu.
unsafe fn system_dpi() -> i32 {
    let hdc = GetDC(ptr::null_mut());
    if hdc.is_null() {
        return 96;
    }
    let dpi = GetDeviceCaps(hdc, LOGPIXELSX as i32);
    ReleaseDC(ptr::null_mut(), hdc);
    if dpi > 0 {
        dpi
    } else {
        96
    }
}

/// Pixels logiques (96 ppp) → pixels de l'écran.
fn scale(logical: i32, dpi: i32) -> i32 {
    (logical * dpi + 48) / 96
}

/// Points typographiques → pixels de l'écran.
fn points(size: i32, dpi: i32) -> i32 {
    (size * dpi + 36) / 72
}

unsafe fn create_window(dpi: i32, fonts: &Fonts) -> Option<Handles> {
    let instance = GetModuleHandleW(ptr::null());
    let px = |logical: i32| scale(logical, dpi);

    let class = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(window_procedure),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: LoadIconW(instance, ICON_RESOURCE),
        hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
        hbrBackground: GetSysColorBrush(COLOR_WINDOW),
        lpszMenuName: ptr::null(),
        lpszClassName: CLASS_NAME,
        hIconSm: LoadImageW(
            instance,
            ICON_RESOURCE,
            IMAGE_ICON,
            px(16),
            px(16),
            LR_DEFAULTCOLOR,
        ),
    };
    if RegisterClassExW(&class) == 0 {
        return None;
    }

    let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    let mut frame = RECT {
        left: 0,
        top: 0,
        right: px(WINDOW_WIDTH),
        bottom: px(WINDOW_HEIGHT),
    };
    AdjustWindowRectEx(&mut frame, style, 0, 0);
    let width = frame.right - frame.left;
    let height = frame.bottom - frame.top;
    let x = ((GetSystemMetrics(SM_CXSCREEN) - width) / 2).max(0);
    let y = ((GetSystemMetrics(SM_CYSCREEN) - height) / 2).max(0);

    let window = CreateWindowExW(
        0,
        CLASS_NAME,
        WINDOW_TITLE,
        style,
        x,
        y,
        width,
        height,
        ptr::null_mut(),
        ptr::null_mut(),
        instance,
        ptr::null(),
    );
    if window.is_null() {
        return None;
    }

    let child = |class: PCWSTR,
                 text: PCWSTR,
                 style: u32,
                 rect: (i32, i32, i32, i32),
                 id: i32,
                 font: HFONT|
     -> HWND {
        let handle = CreateWindowExW(
            0,
            class,
            text,
            WS_CHILD | WS_VISIBLE | style,
            px(rect.0),
            px(rect.1),
            px(rect.2),
            px(rect.3),
            window,
            id as usize as HMENU,
            instance,
            ptr::null(),
        );
        if !handle.is_null() && !font.is_null() {
            SendMessageW(handle, WM_SETFONT, font as usize, 1);
        }
        handle
    };

    let statik = w!("STATIC");
    let button = w!("BUTTON");
    let text = SS_LEFT | SS_NOPREFIX;

    let title = child(
        statik,
        WINDOW_TITLE,
        text | SS_ENDELLIPSIS,
        (TEXT_X, 34, TEXT_WIDTH, 30),
        ID_TITLE,
        fonts.title,
    );
    let intro = child(statik, w!(""), text, (TEXT_X, 70, TEXT_WIDTH, 18), ID_INTRO, fonts.text);
    let path = child(
        statik,
        w!(""),
        text | SS_PATHELLIPSIS,
        (TEXT_X, 90, TEXT_WIDTH, 18),
        ID_PATH,
        fonts.text,
    );
    let desktop = child(
        button,
        w!("Créer un raccourci sur le Bureau"),
        BS_AUTOCHECKBOX as u32 | WS_TABSTOP,
        (MARGIN, 150, FULL_WIDTH, 22),
        ID_DESKTOP,
        fonts.text,
    );
    let note = child(
        statik,
        w!("Un raccourci est toujours ajouté au menu Démarrer."),
        text,
        (MARGIN + 20, 174, FULL_WIDTH - 20, 18),
        ID_NOTE,
        fonts.text,
    );
    let bar = (MARGIN, 214, FULL_WIDTH, 14);
    let marquee = child(PROGRESS_CLASSW, w!(""), PBS_MARQUEE, bar, ID_MARQUEE, ptr::null_mut());
    let progress = child(PROGRESS_CLASSW, w!(""), 0, bar, ID_PROGRESS, ptr::null_mut());
    let status = child(
        statik,
        w!("Connexion au panel…"),
        text | SS_EDITCONTROL,
        (MARGIN, 236, FULL_WIDTH, 56),
        ID_STATUS,
        fonts.text,
    );
    let install = child(
        button,
        w!("Installer"),
        BS_DEFPUSHBUTTON as u32 | WS_TABSTOP,
        (WINDOW_WIDTH - MARGIN - 88 - 8 - 110, BUTTON_Y, 110, BUTTON_HEIGHT),
        ID_INSTALL,
        fonts.text,
    );
    let cancel = child(
        button,
        w!("Annuler"),
        BS_PUSHBUTTON as u32 | WS_TABSTOP,
        (WINDOW_WIDTH - MARGIN - 88, BUTTON_Y, 88, BUTTON_HEIGHT),
        ID_CANCEL,
        fonts.text,
    );

    let handles = Handles {
        window,
        title,
        intro,
        path,
        desktop,
        note,
        marquee,
        progress,
        status,
        install,
        cancel,
        dpi,
    };
    if [
        title, intro, path, desktop, note, marquee, progress, status, install, cancel,
    ]
    .iter()
    .any(|handle| handle.is_null())
    {
        return None;
    }

    // Coché par défaut ; le choix ne s'ouvre qu'une fois le serveur connu.
    SendMessageW(desktop, BM_SETCHECK, BST_CHECKED as usize, 0);
    EnableWindow(desktop, 0);
    EnableWindow(install, 0);
    SendMessageW(marquee, PBM_SETMARQUEE, 1, 30);
    SendMessageW(progress, PBM_SETRANGE32, 0, PROGRESS_RANGE as isize);
    ShowWindow(progress, SW_HIDE);

    Some(handles)
}

// ------------------------------------------------------ fil de travail

/// Le `Report` du fil de travail : écrit l'instantané, prévient la fenêtre.
struct Courier {
    window: usize,
    shared: Arc<Mutex<Snapshot>>,
}

impl Courier {
    fn update(&self, change: impl FnOnce(&mut Snapshot)) {
        change(&mut lock(&self.shared));
        self.post(WM_APP_REFRESH);
    }

    fn finish(&self, outcome: Outcome, message: u32) {
        lock(&self.shared).outcome = Some(outcome);
        self.post(message);
    }

    fn post(&self, message: u32) {
        // SÛRETÉ : `PostMessageW` est prévu pour être appelé depuis un autre
        // fil ; une fenêtre déjà détruite fait simplement échouer l'appel.
        unsafe { PostMessageW(self.window as HWND, message, 0, 0) };
    }
}

impl Report for Courier {
    fn step(&self, index: u8, label: &str) {
        let step = format!("Étape {index} sur {STEPS} : {label}");
        self.update(|snapshot| {
            snapshot.step = step;
            snapshot.detail.clear();
            snapshot.transfer = None;
        });
    }

    fn detail(&self, text: &str) {
        let text = text.to_owned();
        self.update(|snapshot| snapshot.detail = text);
    }

    fn transfer(&self, label: &str, done: u64, total: u64) {
        let transfer = Transfer {
            label: label.to_owned(),
            done,
            total,
        };
        self.update(|snapshot| snapshot.transfer = Some(transfer));
    }

    fn transfer_done(&self) {
        self.update(|snapshot| snapshot.transfer = None);
    }
}

unsafe fn start_prepare(handles: Handles) {
    let shared = with_state(|state| {
        state.stage = Stage::Preparing;
        state.prepared = None;
        state.last_status = "Connexion au panel…".to_owned();
        *lock(&state.shared) = Snapshot::default();
        state.shared.clone()
    });
    set_text(handles.status, "Connexion au panel…");
    EnableWindow(handles.desktop, 0);
    EnableWindow(handles.install, 0);
    set_text(handles.cancel, "Annuler");
    show_bars(handles, Bars::Marquee);

    let courier = Courier {
        window: handles.window as usize,
        shared,
    };
    thread::spawn(move || {
        let http = Http::new();
        let outcome = install::prepare(&http, &courier).map(|prepared| {
            let logo = logo::fetch(&http, &prepared.manifest);
            (prepared, logo)
        });
        courier.finish(Outcome::Prepared(outcome), WM_APP_PREPARED);
    });
}

unsafe fn start_install(handles: Handles) {
    let options = Options {
        desktop_shortcut: SendMessageW(handles.desktop, BM_GETCHECK, 0, 0) as u32 == BST_CHECKED,
    };
    let Some((prepared, shared)) = with_state(|state| {
        let prepared = state.prepared.clone()?;
        state.stage = Stage::Installing;
        Some((prepared, state.shared.clone()))
    }) else {
        return;
    };
    EnableWindow(handles.desktop, 0);
    EnableWindow(handles.install, 0);
    show_bars(handles, Bars::Marquee);

    let courier = Courier {
        window: handles.window as usize,
        shared,
    };
    thread::spawn(move || {
        let http = Http::new();
        let outcome = install::install(&http, &prepared, &options, &courier)
            .and_then(|target| install::launch(&prepared, &target, &courier));
        courier.finish(Outcome::Installed(outcome), WM_APP_INSTALLED);
    });
}

fn take_outcome() -> Option<Outcome> {
    with_state(|state| lock(&state.shared).outcome.take())
}

// ---------------------------------------------------------- messages

unsafe extern "system" fn window_procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let Some(handles) = HANDLES.with(Cell::get) else {
        return DefWindowProcW(window, message, wparam, lparam);
    };

    match message {
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as i32;
            let code = (wparam >> 16) as u32;
            match (id, code) {
                (ID_INSTALL, BN_CLICKED) => on_install(handles),
                (ID_CANCEL, _) => {
                    DestroyWindow(handles.window);
                }
                _ => {}
            }
            0
        }
        // Entrée active « Installer » quand il est disponible.
        DM_GETDEFID => match stage() {
            Stage::Ready | Stage::Failed => ((DC_HASDEFID as isize) << 16) | ID_INSTALL as isize,
            _ => 0,
        },
        WM_CTLCOLORSTATIC => control_colour(handles, wparam as HDC, lparam as HWND),
        WM_PAINT => {
            paint(handles);
            0
        }
        WM_APP_REFRESH => {
            refresh(handles);
            0
        }
        WM_APP_PREPARED => {
            on_prepared(handles);
            0
        }
        WM_APP_INSTALLED => {
            on_installed(handles);
            0
        }
        WM_CLOSE => {
            DestroyWindow(window);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(window, message, wparam, lparam),
    }
}

/// Texte sur fond de fenêtre : gris pour ce qui est secondaire, rouge pour
/// une erreur.
unsafe fn control_colour(handles: Handles, hdc: HDC, control: HWND) -> LRESULT {
    SetBkMode(hdc, TRANSPARENT as i32);
    if control == handles.intro || control == handles.path || control == handles.note {
        SetTextColor(hdc, GREY);
    } else if control == handles.status && stage() == Stage::Failed {
        SetTextColor(hdc, RED);
    }
    GetSysColorBrush(COLOR_WINDOW) as LRESULT
}

fn logo_rect(dpi: i32) -> RECT {
    RECT {
        left: scale(MARGIN, dpi),
        top: scale(MARGIN, dpi),
        right: scale(MARGIN + LOGO_SIZE, dpi),
        bottom: scale(MARGIN + LOGO_SIZE, dpi),
    }
}

unsafe fn paint(handles: Handles) {
    let mut paint = PAINTSTRUCT::default();
    // `BeginPaint` peut renvoyer un message d'effacement : hors emprunt.
    let hdc = BeginPaint(handles.window, &mut paint);
    if hdc.is_null() {
        return;
    }
    let rect = logo_rect(handles.dpi);
    STATE.with(|state| {
        if let Ok(state) = state.try_borrow() {
            if let Some(logo) = state.as_ref().and_then(|state| state.logo.as_ref()) {
                logo.draw(
                    hdc,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                );
            }
        }
    });
    EndPaint(handles.window, &paint);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bars {
    Hidden,
    Marquee,
    Progress(i32),
}

unsafe fn show_bars(handles: Handles, bars: Bars) {
    match bars {
        Bars::Hidden => {
            ShowWindow(handles.marquee, SW_HIDE);
            ShowWindow(handles.progress, SW_HIDE);
        }
        Bars::Marquee => {
            ShowWindow(handles.progress, SW_HIDE);
            ShowWindow(handles.marquee, SW_SHOW);
        }
        Bars::Progress(position) => {
            ShowWindow(handles.marquee, SW_HIDE);
            SendMessageW(handles.progress, PBM_SETPOS, position as usize, 0);
            ShowWindow(handles.progress, SW_SHOW);
        }
    }
}

/// L'instantané a changé : texte d'état et barre de progression.
unsafe fn refresh(handles: Handles) {
    let (text, transfer) = with_state(|state| {
        let snapshot = lock(&state.shared);
        let second_line = match &snapshot.transfer {
            Some(transfer) => report::transfer_text(&transfer.label, transfer.done, transfer.total),
            None => snapshot.detail.clone(),
        };
        let text = if second_line.is_empty() {
            snapshot.step.clone()
        } else {
            format!("{}\n{second_line}", snapshot.step)
        };
        (text, snapshot.transfer.clone())
    });

    let changed = with_state(|state| {
        if state.last_status == text {
            false
        } else {
            state.last_status = text.clone();
            true
        }
    });
    if changed {
        set_text(handles.status, &text);
    }

    let bars = match transfer {
        Some(transfer) if transfer.total > 0 => {
            let position = (transfer.done.saturating_mul(PROGRESS_RANGE as u64) / transfer.total)
                .min(PROGRESS_RANGE as u64) as i32;
            Bars::Progress(position)
        }
        _ => match stage() {
            Stage::Preparing | Stage::Installing => Bars::Marquee,
            Stage::Ready | Stage::Failed => Bars::Hidden,
        },
    };
    show_bars(handles, bars);
}

unsafe fn on_prepared(handles: Handles) {
    match take_outcome() {
        Some(Outcome::Prepared(Ok((prepared, logo)))) => ready(handles, prepared, logo),
        Some(Outcome::Prepared(Err(error))) => fail(handles, &error),
        _ => {}
    }
}

unsafe fn on_installed(handles: Handles) {
    match take_outcome() {
        Some(Outcome::Installed(Ok(()))) => {
            with_state(|state| state.launched = true);
            DestroyWindow(handles.window);
        }
        Some(Outcome::Installed(Err(error))) => fail(handles, &error),
        _ => {}
    }
}

unsafe fn on_install(handles: Handles) {
    match stage() {
        Stage::Ready => start_install(handles),
        // Repartir du début : le manifeste a pu changer entre-temps.
        Stage::Failed => start_prepare(handles),
        Stage::Preparing | Stage::Installing => {}
    }
}

/// Le serveur est connu : nom, logo, dossier, et la main au joueur.
unsafe fn ready(handles: Handles, prepared: Prepared, logo: Option<Vec<u8>>) {
    let name = prepared.manifest.display_name().to_owned();
    let root = prepared.layout.root.display().to_string();
    let image = logo.as_deref().and_then(Image::from_png);
    let has_logo = image.is_some();

    let icons = with_state(|state| {
        state.stage = Stage::Ready;
        state.prepared = Some(Arc::new(prepared));
        state.last_status = "Prêt à installer.".to_owned();
        if let Some(image) = image {
            state.logo = Some(image);
        }
        state.logo.as_ref().filter(|_| has_logo).map(|logo| {
            (
                logo.to_icon(scale(32, handles.dpi)),
                logo.to_icon(scale(16, handles.dpi)),
            )
        })
    });

    set_text(handles.window, &format!("Installation de {name}"));
    set_text(handles.title, &name);
    set_text(handles.intro, "Le launcher sera installé dans :");
    set_text(handles.path, &root);
    set_text(handles.status, "Prêt à installer.");
    if let Some((big, small)) = icons {
        set_icons(handles, big, small);
        InvalidateRect(handles.window, &logo_rect(handles.dpi), 1);
    }

    show_bars(handles, Bars::Hidden);
    EnableWindow(handles.desktop, 1);
    EnableWindow(handles.install, 1);
    set_text(handles.install, "Installer");
    set_text(handles.cancel, "Annuler");
    SetFocus(handles.install);
}

/// Quelque chose a échoué : le message et son conseil, et de quoi réessayer.
unsafe fn fail(handles: Handles, error: &BootstrapError) {
    let text = error.to_string();
    let known = with_state(|state| {
        state.stage = Stage::Failed;
        state.last_status = text.clone();
        state.prepared.is_some()
    });
    set_text(handles.status, &text);
    show_bars(handles, Bars::Hidden);
    EnableWindow(handles.desktop, i32::from(known));
    EnableWindow(handles.install, 1);
    set_text(handles.install, "Réessayer");
    set_text(handles.cancel, "Fermer");
    SetFocus(handles.install);
}

unsafe fn set_icons(handles: Handles, big: HICON, small: HICON) {
    if !big.is_null() {
        SendMessageW(handles.window, WM_SETICON, ICON_BIG as usize, big as LPARAM);
    }
    if !small.is_null() {
        SendMessageW(handles.window, WM_SETICON, ICON_SMALL as usize, small as LPARAM);
    }
}

unsafe fn set_text(control: HWND, text: &str) {
    let text = wide(text);
    SetWindowTextW(control, text.as_ptr());
}
