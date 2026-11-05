#![windows_subsystem = "windows"]
#![cfg_attr(not(windows), allow(dead_code))]

//! Proton App Launcher: a tiny Win32 GUI that launches one of the applications
//! discovered under `C:\proton_shortcuts`.
//!
//! Build target: `x86_64-pc-windows-gnu`. No third-party crates are used.

mod config;
mod desktop;
#[cfg(windows)]
mod win32;

#[cfg(windows)]
use config::LauncherConfig;
#[cfg(windows)]
use std::path::PathBuf;
#[cfg(windows)]
use std::sync::OnceLock;

#[cfg(windows)]
static CONFIG: OnceLock<LauncherConfig> = OnceLock::new();
#[cfg(windows)]
static CONFIG_DIR: OnceLock<PathBuf> = OnceLock::new();
#[cfg(windows)]
static ICONS: OnceLock<Vec<usize>> = OnceLock::new();

#[cfg(windows)]
const ID_LAUNCH: i32 = 1;
#[cfg(windows)]
const ID_CANCEL: i32 = 2;
#[cfg(windows)]
const ID_LABEL: i32 = 3;
#[cfg(windows)]
const ID_RADIO_BASE: i32 = 1000;

#[cfg(windows)]
fn main() {
    if let Err(message) = run() {
        unsafe {
            win32::message_box(
                std::ptr::null_mut(),
                &message,
                "Proton App Launcher",
                win32::MB_OK | win32::MB_ICONERROR,
            );
        }
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("launcher.exe must be built for Windows (x86_64-pc-windows-gnu).");
}

#[cfg(windows)]
fn base_dir() -> PathBuf {
    // Prefer the executable's own directory when it is the shortcuts folder,
    // otherwise fall back to the fixed Wine drive-C path.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let is_shortcuts_dir = dir
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.eq_ignore_ascii_case("proton_shortcuts"))
                .unwrap_or(false);
            if is_shortcuts_dir {
                return dir.to_path_buf();
            }
        }
    }
    PathBuf::from(r"C:\proton_shortcuts")
}

#[cfg(windows)]
fn run() -> Result<(), String> {
    use win32::*;

    let dir = base_dir();
    let config = LauncherConfig::load_or_create(&dir)?;
    let _ = CONFIG_DIR.set(dir.clone());
    if config.apps.is_empty() {
        return Err(format!(
            "No applications configured in {}.\n\nEdit {}\\config.yaml or add .desktop files and run the launcher again.",
            dir.display(),
            dir.display()
        ));
    }
    let app_names: Vec<String> = config.apps.iter().map(|app| app.name.clone()).collect();

    // GDI+ stays alive until the message loop has finished; icon handles must
    // not outlive the loader that created them. Skip it entirely when no app
    // has an icon so prefixes without gdiplus.dll keep the old startup path.
    let gdiplus = if config.apps.iter().any(|app| app.icon.is_some()) {
        Gdiplus::load()
    } else {
        None
    };
    let icon_handles = unsafe { load_icons(gdiplus.as_ref(), &dir, &config) };
    let window_icon = icon_handles
        .iter()
        .copied()
        .find(|&handle| handle != 0)
        .map(|handle| handle as HICON)
        .unwrap_or(std::ptr::null_mut());
    let show_icons = icon_handles.iter().any(|&handle| handle != 0);
    let _ = ICONS.set(icon_handles);

    CONFIG
        .set(config)
        .map_err(|_| "configuration was already initialised".to_string())?;

    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        if instance.is_null() {
            return Err(format!(
                "GetModuleHandleW failed (error {})",
                GetLastError()
            ));
        }

        let class_name = wide("ProtonAppLauncher");
        let window_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: window_icon,
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW as LPCWSTR),
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: window_icon,
        };
        if RegisterClassExW(&window_class) == 0 {
            return Err(format!(
                "RegisterClassExW failed (error {})",
                GetLastError()
            ));
        }

        let icon_column = if show_icons { 40 } else { 0 };
        let row_height = if show_icons { 34 } else { 26 };
        let radio_height = row_height * app_names.len() as i32;
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 440 + icon_column,
            bottom: 96 + radio_height,
        };
        AdjustWindowRectEx(&mut rect, WS_OVERLAPPEDWINDOW, 0, 0);
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;

        let title = wide("Proton App Launcher");
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            width,
            height,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        );
        if hwnd.is_null() {
            return Err(format!("CreateWindowExW failed (error {})", GetLastError()));
        }

        let label_text = wide("Select an application, then press Launch (Enter).");
        create_child(
            hwnd,
            instance,
            "STATIC",
            &label_text,
            WS_CHILD | WS_VISIBLE | SS_LEFT,
            ID_LABEL,
        );

        for (index, name) in app_names.iter().enumerate() {
            let id = ID_RADIO_BASE + index as i32;
            let text = wide(name);
            let radio = create_child(
                hwnd,
                instance,
                "BUTTON",
                &text,
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | BS_AUTORADIOBUTTON
                    | if index == 0 { WS_GROUP } else { 0 },
                id,
            );
            if index == 0 {
                SendMessageW(radio, BM_SETCHECK, BST_CHECKED as WPARAM, 0);
            }
        }

        let launch_text = wide("Launch");
        create_child(
            hwnd,
            instance,
            "BUTTON",
            &launch_text,
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_GROUP | BS_DEFPUSHBUTTON,
            ID_LAUNCH,
        );
        let cancel_text = wide("Cancel");
        create_child(
            hwnd,
            instance,
            "BUTTON",
            &cancel_text,
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_GROUP | BS_PUSHBUTTON,
            ID_CANCEL,
        );

        layout_controls(hwnd, app_names.len());
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

        let mut message = MSG::default();
        loop {
            let result = GetMessageW(&mut message, std::ptr::null_mut(), 0, 0);
            if result <= 0 {
                break;
            }
            // Enter launches, Escape cancels. Handling this before dispatch
            // keeps the keyboard behaviour independent of which control has
            // focus and needs no dialog manager.
            if message.message == WM_KEYDOWN {
                match message.wParam {
                    VK_ESCAPE => {
                        PostQuitMessage(0);
                        break;
                    }
                    VK_RETURN => {
                        launch_selected(hwnd);
                        continue;
                    }
                    _ => {}
                }
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    drop(gdiplus);
    Ok(())
}

#[cfg(windows)]
unsafe fn load_icons(
    gdiplus: Option<&win32::Gdiplus>,
    dir: &std::path::Path,
    config: &LauncherConfig,
) -> Vec<usize> {
    config
        .apps
        .iter()
        .map(|app| {
            let Some(icon) = app.icon.as_deref() else {
                return 0;
            };
            let Some(gdiplus) = gdiplus else {
                return 0;
            };
            for candidate in desktop::icon_candidates(dir, icon) {
                if let Some(handle) = gdiplus.load_icon(&candidate) {
                    return handle as usize;
                }
            }
            0
        })
        .collect()
}

#[cfg(windows)]
unsafe fn create_child(
    parent: win32::HWND,
    instance: win32::HINSTANCE,
    class: &str,
    text: &[u16],
    style: u32,
    id: i32,
) -> win32::HWND {
    use win32::*;
    let class = wide(class);
    CreateWindowExW(
        0,
        class.as_ptr(),
        text.as_ptr(),
        style,
        0,
        0,
        10,
        10,
        parent,
        id as usize as HMENU,
        instance,
        std::ptr::null_mut(),
    )
}

#[cfg(windows)]
unsafe fn layout_controls(hwnd: win32::HWND, app_count: usize) {
    use win32::*;

    let mut client = RECT::default();
    GetClientRect(hwnd, &mut client);
    let margin = 16;
    let full_width = (client.right - client.left) - 2 * margin;
    let button_y = (client.bottom - margin) - 32;
    let show_icons = ICONS
        .get()
        .is_some_and(|icons| icons.iter().any(|&handle| handle != 0));
    let icon_column = if show_icons { 40 } else { 0 };
    let row_height = if show_icons { 34 } else { 26 };

    MoveWindow(GetDlgItem(hwnd, ID_LABEL), margin, 14, full_width, 20, 1);

    let mut y = 42;
    for index in 0..app_count {
        MoveWindow(
            GetDlgItem(hwnd, ID_RADIO_BASE + index as i32),
            margin + icon_column,
            y,
            full_width - icon_column,
            if show_icons { 24 } else { 22 },
            1,
        );
        y += row_height;
    }

    MoveWindow(
        GetDlgItem(hwnd, ID_LAUNCH),
        margin + full_width - 200,
        button_y,
        94,
        32,
        1,
    );
    MoveWindow(
        GetDlgItem(hwnd, ID_CANCEL),
        margin + full_width - 100,
        button_y,
        100,
        32,
        1,
    );
}

#[cfg(windows)]
unsafe extern "system" fn wndproc(
    hwnd: win32::HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use win32::*;
    match message {
        WM_COMMAND => {
            match (wparam & 0xFFFF) as i32 {
                ID_LAUNCH => launch_selected(hwnd),
                ID_CANCEL => PostQuitMessage(0),
                _ => {}
            }
            0
        }
        WM_SIZE => {
            if let Some(config) = CONFIG.get() {
                layout_controls(hwnd, config.apps.len());
            }
            InvalidateRect(hwnd, std::ptr::null(), 1);
            0
        }
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut paint);
            FillRect(hdc, &paint.rcPaint, (COLOR_WINDOW + 1) as HBRUSH);
            if let Some(icons) = ICONS.get() {
                if icons.iter().any(|&handle| handle != 0) {
                    let margin = 16;
                    let mut y = 42;
                    for &handle in icons {
                        if handle != 0 {
                            DrawIconEx(
                                hdc,
                                margin,
                                y,
                                handle as HICON,
                                32,
                                32,
                                0,
                                std::ptr::null_mut(),
                                DI_NORMAL,
                            );
                        }
                        y += 34;
                    }
                }
            }
            EndPaint(hwnd, &paint);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

#[cfg(windows)]
unsafe fn launch_selected(hwnd: win32::HWND) {
    use std::process::Command;
    use win32::*;

    let Some(config) = CONFIG.get() else {
        return;
    };
    let selected =
        config.apps.iter().enumerate().find(|(index, _)| {
            IsDlgButtonChecked(hwnd, ID_RADIO_BASE + *index as i32) == BST_CHECKED
        });
    let Some((index, _)) = selected else {
        return;
    };

    // Re-read the file so edits made while the window is open take effect
    // without restarting. If the app list itself changed, keep using the
    // cached entry: the radio buttons still show the old list.
    let app = CONFIG_DIR
        .get()
        .and_then(|dir| LauncherConfig::load_existing(dir))
        .filter(|fresh| fresh.apps.len() == config.apps.len())
        .and_then(|fresh| fresh.apps.into_iter().nth(index))
        .unwrap_or_else(|| config.apps[index].clone());

    let path = desktop::normalize_windows_path(&app.path);
    let mut command;
    let mut shortcut_cwd = None;

    if path.to_ascii_lowercase().ends_with(".lnk") {
        match resolve_shortcut(&path) {
            Some(shortcut) => {
                command = Command::new(desktop::normalize_windows_path(&shortcut.target));
                command.args(&app.args);
                if !shortcut.args.is_empty() {
                    use std::os::windows::process::CommandExt;
                    command.raw_arg(&shortcut.args);
                }
                if !shortcut.cwd.is_empty() {
                    shortcut_cwd = Some(desktop::normalize_windows_path(&shortcut.cwd));
                }
            }
            // Advertised or exotic shortcuts may not resolve to a path; let
            // the shell open those instead.
            None => {
                command = Command::new("explorer.exe");
                command.arg(&path);
            }
        }
    } else {
        command = Command::new(&path);
        command.args(&app.args);
    }

    let cwd = app
        .cwd
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(|value| desktop::windows_cwd(value).unwrap_or_else(|| value.to_string()))
        .or(shortcut_cwd);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    for (key, value) in &app.env {
        command.env(key, value);
    }
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    match command.spawn() {
        // Stay open so another entry can be launched.
        Ok(_) => {}
        Err(error) => {
            let message = format!(
                "Failed to launch \"{}\":\n{}\n\nPath: {}\nCWD: {}",
                app.name,
                error,
                path,
                app.cwd.as_deref().unwrap_or("(inherited)")
            );
            message_box(hwnd, &message, "Proton App Launcher", MB_OK | MB_ICONERROR);
        }
    }
}

struct Shortcut {
    target: String,
    args: String,
    cwd: String,
}

/// Resolves a `.lnk` target with `IShellLinkW`, so the target can be started
/// with the configured arguments, working directory and environment.
///
/// `ole32.dll` is loaded at runtime; only `user32` and `kernel32` are linked.
#[cfg(windows)]
unsafe fn resolve_shortcut(path: &str) -> Option<Shortcut> {
    use std::ffi::c_void;
    use win32::*;

    const COINIT_APARTMENTTHREADED: u32 = 0x0002;
    const CLSCTX_INPROC_SERVER: u32 = 0x0001;
    const STGM_READ: u32 = 0x0000;
    const SLGP_DEFAULT: u32 = 0x0000;
    const SLR_NO_UI: u32 = 0x0001;
    const RPC_E_CHANGED_MODE: HRESULT = 0x8001_0106u32 as i32;

    let ole = Ole32::load()?;
    let hr = (ole.co_initialize_ex)(std::ptr::null_mut(), COINIT_APARTMENTTHREADED);
    if hr < 0 && hr != RPC_E_CHANGED_MODE {
        return None;
    }
    let initialized = hr >= 0;

    let mut link: *mut c_void = std::ptr::null_mut();
    let hr = (ole.co_create_instance)(
        &CLSID_SHELL_LINK,
        std::ptr::null_mut(),
        CLSCTX_INPROC_SERVER,
        &IID_ISHELL_LINKW,
        &mut link,
    );
    let mut resolved = None;
    if hr >= 0 && !link.is_null() {
        let link_vtbl = *(link as *mut *const IUnknownVtbl);
        let mut persist: *mut c_void = std::ptr::null_mut();
        let hr = ((*link_vtbl).query_interface)(link, &IID_IPERSIST_FILE, &mut persist);
        if hr >= 0 && !persist.is_null() {
            let persist_vtbl = *(persist as *mut *const IPersistFileVtbl);
            let wide_path = wide(path);
            if ((*persist_vtbl).load)(persist, wide_path.as_ptr(), STGM_READ) >= 0 {
                resolved = read_link_target(link, SLGP_DEFAULT, SLR_NO_UI);
            }
            ((*persist_vtbl).base.release)(persist);
        }
        ((*link_vtbl).release)(link);
    }
    if initialized {
        (ole.co_uninitialize)();
    }
    resolved
}

#[cfg(windows)]
unsafe fn read_link_target(
    link: *mut std::ffi::c_void,
    path_flags: u32,
    resolve_flags: u32,
) -> Option<Shortcut> {
    use win32::*;

    let vtbl = *(link as *mut *const IShellLinkWVtbl);
    let mut target = vec![0u16; 32768];
    let mut hr = ((*vtbl).get_path)(
        link,
        target.as_mut_ptr(),
        target.len() as i32,
        std::ptr::null_mut(),
        path_flags,
    );
    if hr < 0 || target[0] == 0 {
        // Relative or IDList-only shortcuts need an explicit resolve.
        let _ = ((*vtbl).resolve)(link, std::ptr::null_mut(), resolve_flags);
        hr = ((*vtbl).get_path)(
            link,
            target.as_mut_ptr(),
            target.len() as i32,
            std::ptr::null_mut(),
            path_flags,
        );
    }
    if hr < 0 {
        return None;
    }
    let target = utf16z(&target);
    if target.is_empty() {
        return None;
    }

    let mut args_buf = vec![0u16; 32768];
    let args = if ((*vtbl).get_arguments)(link, args_buf.as_mut_ptr(), args_buf.len() as i32) >= 0 {
        utf16z(&args_buf)
    } else {
        String::new()
    };
    let mut cwd_buf = vec![0u16; 4096];
    let cwd =
        if ((*vtbl).get_working_directory)(link, cwd_buf.as_mut_ptr(), cwd_buf.len() as i32) >= 0 {
            utf16z(&cwd_buf)
        } else {
            String::new()
        };

    Some(Shortcut { target, args, cwd })
}

fn utf16z(buffer: &[u16]) -> String {
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len])
}
