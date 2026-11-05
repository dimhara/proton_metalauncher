//! Raw Win32 FFI bindings. Only `user32` and `kernel32` are linked; no crates
//! and no generated bindings.

#![allow(
    dead_code,
    non_snake_case,
    non_camel_case_types,
    clippy::upper_case_acronyms
)]

use std::ffi::c_void;
use std::path::Path;

pub type HWND = *mut c_void;
pub type HINSTANCE = *mut c_void;
pub type HMODULE = *mut c_void;
pub type HMENU = *mut c_void;
pub type HICON = *mut c_void;
pub type HCURSOR = *mut c_void;
pub type HBRUSH = *mut c_void;
pub type HDC = *mut c_void;
pub type HANDLE = *mut c_void;
pub type LPCWSTR = *const u16;
pub type WPARAM = usize;
pub type LPARAM = isize;
pub type LRESULT = isize;
pub type BOOL = i32;
pub type UINT = u32;
pub type DWORD = u32;
pub type ATOM = u16;

pub type WNDPROC = Option<unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT>;

#[repr(C)]
#[derive(Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
#[derive(Default)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: BOOL,
    pub rcPaint: RECT,
    pub fRestore: BOOL,
    pub fIncUpdate: BOOL,
    pub rgbReserved: [u8; 32],
}

#[repr(C)]
#[derive(Default)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: UINT,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: DWORD,
    pub pt: POINT,
}

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: UINT,
    pub style: UINT,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: LPCWSTR,
    pub lpszClassName: LPCWSTR,
    pub hIconSm: HICON,
}

// Window styles
pub const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
pub const WS_CHILD: u32 = 0x4000_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;
pub const WS_TABSTOP: u32 = 0x0001_0000;
pub const WS_GROUP: u32 = 0x0002_0000;

// Button / static styles
pub const BS_PUSHBUTTON: u32 = 0x0000_0000;
pub const BS_DEFPUSHBUTTON: u32 = 0x0000_0001;
pub const BS_AUTORADIOBUTTON: u32 = 0x0000_0009;
pub const SS_LEFT: u32 = 0x0000_0000;

// Messages
pub const WM_CREATE: u32 = 0x0001;
pub const WM_SIZE: u32 = 0x0005;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_COMMAND: u32 = 0x0111;

// Button messages / states
pub const BM_GETCHECK: u32 = 0x00F0;
pub const BM_SETCHECK: u32 = 0x00F1;
pub const BST_CHECKED: u32 = 0x0001;
pub const BST_UNCHECKED: u32 = 0x0000;
pub const BN_CLICKED: u16 = 0;

// Misc
pub const SW_SHOW: i32 = 5;
pub const IDC_ARROW: usize = 32512;
pub const COLOR_WINDOW: usize = 5;
pub const CW_USEDEFAULT: i32 = 0x8000_0000u32 as i32;
pub const MB_OK: u32 = 0x0000_0000;
pub const MB_ICONERROR: u32 = 0x0000_0010;
pub const MB_ICONINFORMATION: u32 = 0x0000_0040;

// Image / icon drawing
pub const IMAGE_ICON: u32 = 1;
pub const LR_LOADFROMFILE: u32 = 0x0000_0010;
pub const DI_NORMAL: u32 = 0x0003;

// Virtual key codes
pub const VK_RETURN: usize = 0x0D;
pub const VK_ESCAPE: usize = 0x1B;

#[link(name = "user32")]
extern "system" {
    pub fn RegisterClassExW(lpWndClass: *const WNDCLASSEXW) -> ATOM;
    pub fn CreateWindowExW(
        dwExStyle: DWORD,
        lpClassName: LPCWSTR,
        lpWindowName: LPCWSTR,
        dwStyle: DWORD,
        x: i32,
        y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: HMENU,
        hInstance: HINSTANCE,
        lpParam: *mut c_void,
    ) -> HWND;
    pub fn DefWindowProcW(hWnd: HWND, msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn DestroyWindow(hWnd: HWND) -> BOOL;
    pub fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
    pub fn UpdateWindow(hWnd: HWND) -> BOOL;
    pub fn GetMessageW(
        lpMsg: *mut MSG,
        hWnd: HWND,
        wMsgFilterMin: UINT,
        wMsgFilterMax: UINT,
    ) -> BOOL;
    pub fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    pub fn PostQuitMessage(nExitCode: i32);
    pub fn SendMessageW(hWnd: HWND, msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn GetDlgItem(hDlg: HWND, nIDDlgItem: i32) -> HWND;
    pub fn IsDlgButtonChecked(hDlg: HWND, nIDButton: i32) -> UINT;
    pub fn MoveWindow(
        hWnd: HWND,
        x: i32,
        y: i32,
        nWidth: i32,
        nHeight: i32,
        bRepaint: BOOL,
    ) -> BOOL;
    pub fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
    pub fn AdjustWindowRectEx(
        lpRect: *mut RECT,
        dwStyle: DWORD,
        bMenu: BOOL,
        dwExStyle: DWORD,
    ) -> BOOL;
    pub fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: LPCWSTR) -> HCURSOR;
    pub fn MessageBoxW(hWnd: HWND, lpText: LPCWSTR, lpCaption: LPCWSTR, uType: UINT) -> i32;
    pub fn BeginPaint(hWnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;
    pub fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> BOOL;
    pub fn FillRect(hDC: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    pub fn DrawIconEx(
        hdc: HDC,
        xLeft: i32,
        yTop: i32,
        hIcon: HICON,
        cxWidth: i32,
        cyWidth: i32,
        istepIfAniCur: UINT,
        hbrFlickerFreeDraw: HBRUSH,
        diFlags: UINT,
    ) -> BOOL;
    pub fn InvalidateRect(hWnd: HWND, lpRect: *const RECT, bErase: BOOL) -> BOOL;
    pub fn LoadImageW(
        hInst: HINSTANCE,
        name: LPCWSTR,
        type_: UINT,
        cx: i32,
        cy: i32,
        fuLoad: UINT,
    ) -> HANDLE;
}

#[link(name = "kernel32")]
extern "system" {
    pub fn GetModuleHandleW(lpModuleName: LPCWSTR) -> HMODULE;
    pub fn GetLastError() -> DWORD;
    pub fn LoadLibraryW(lpFileName: LPCWSTR) -> HMODULE;
    pub fn GetProcAddress(hModule: HMODULE, lpProcName: *const u8) -> *mut c_void;
    pub fn FreeLibrary(hModule: HMODULE) -> BOOL;
}

/// Converts a Rust string to a NUL-terminated UTF-16 buffer.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Shows a modal message box.
pub unsafe fn message_box(owner: HWND, text: &str, caption: &str, flags: u32) {
    let text = wide(text);
    let caption = wide(caption);
    MessageBoxW(owner, text.as_ptr(), caption.as_ptr(), flags);
}

pub type HRESULT = i32;

/// COM GUID (`{data1-data2-data3-data4}`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

/// `CLSID_ShellLink`.
pub const CLSID_SHELL_LINK: GUID = GUID {
    data1: 0x0002_1401,
    data2: 0,
    data3: 0,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

/// `IID_IShellLinkW`.
pub const IID_ISHELL_LINKW: GUID = GUID {
    data1: 0x0002_14F9,
    data2: 0,
    data3: 0,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

/// `IID_IPersistFile`.
pub const IID_IPERSIST_FILE: GUID = GUID {
    data1: 0x0000_010B,
    data2: 0,
    data3: 0,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

#[repr(C)]
pub struct IUnknownVtbl {
    pub query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    pub release: unsafe extern "system" fn(*mut c_void) -> u32,
}

/// Prefix of the `IShellLinkW` vtable up to `GetArguments`.
#[repr(C)]
pub struct IShellLinkWVtbl {
    pub base: IUnknownVtbl,
    pub get_path:
        unsafe extern "system" fn(*mut c_void, *mut u16, i32, *mut c_void, u32) -> HRESULT,
    pub get_id_list: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub set_id_list: unsafe extern "system" fn(*mut c_void, *const c_void) -> HRESULT,
    pub get_description: unsafe extern "system" fn(*mut c_void, *mut u16, i32) -> HRESULT,
    pub set_description: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    pub get_working_directory: unsafe extern "system" fn(*mut c_void, *mut u16, i32) -> HRESULT,
    pub set_working_directory: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    pub get_arguments: unsafe extern "system" fn(*mut c_void, *mut u16, i32) -> HRESULT,
    pub set_arguments: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    pub get_hotkey: unsafe extern "system" fn(*mut c_void, *mut u16) -> HRESULT,
    pub set_hotkey: unsafe extern "system" fn(*mut c_void, u16) -> HRESULT,
    pub get_show_cmd: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
    pub set_show_cmd: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
    pub get_icon_location:
        unsafe extern "system" fn(*mut c_void, *mut u16, i32, *mut i32) -> HRESULT,
    pub set_icon_location: unsafe extern "system" fn(*mut c_void, *const u16, i32) -> HRESULT,
    pub set_relative_path: unsafe extern "system" fn(*mut c_void, *const u16, u32) -> HRESULT,
    pub resolve: unsafe extern "system" fn(*mut c_void, HWND, u32) -> HRESULT,
    pub set_path: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
}

/// Prefix of the `IPersistFile` vtable up to `Load`.
#[repr(C)]
pub struct IPersistFileVtbl {
    pub base: IUnknownVtbl,
    pub get_class_id: unsafe extern "system" fn(*mut c_void, *mut GUID) -> HRESULT,
    pub is_dirty: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    pub load: unsafe extern "system" fn(*mut c_void, *const u16, u32) -> HRESULT,
}

pub type CoInitializeExFn = unsafe extern "system" fn(*mut c_void, u32) -> HRESULT;
pub type CoCreateInstanceFn = unsafe extern "system" fn(
    *const GUID,
    *mut c_void,
    u32,
    *const GUID,
    *mut *mut c_void,
) -> HRESULT;
pub type CoUninitializeFn = unsafe extern "system" fn();

/// `ole32.dll` loaded on demand so the import table stays `user32`+`kernel32`.
pub struct Ole32 {
    module: HMODULE,
    pub co_initialize_ex: CoInitializeExFn,
    pub co_create_instance: CoCreateInstanceFn,
    pub co_uninitialize: CoUninitializeFn,
}

impl Ole32 {
    pub fn load() -> Option<Ole32> {
        unsafe {
            let module = LoadLibraryW(wide("ole32.dll").as_ptr());
            if module.is_null() {
                return None;
            }
            let init = GetProcAddress(module, c"CoInitializeEx".as_ptr().cast::<u8>());
            let create = GetProcAddress(module, c"CoCreateInstance".as_ptr().cast::<u8>());
            let uninit = GetProcAddress(module, c"CoUninitialize".as_ptr().cast::<u8>());
            if init.is_null() || create.is_null() || uninit.is_null() {
                FreeLibrary(module);
                return None;
            }
            Some(Ole32 {
                module,
                co_initialize_ex: std::mem::transmute::<*mut c_void, CoInitializeExFn>(init),
                co_create_instance: std::mem::transmute::<*mut c_void, CoCreateInstanceFn>(create),
                co_uninitialize: std::mem::transmute::<*mut c_void, CoUninitializeFn>(uninit),
            })
        }
    }
}

impl Drop for Ole32 {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.module);
        }
    }
}

/// `GdiplusStartupInput` with `GdiplusVersion = 1`.
#[repr(C)]
pub struct GdiplusStartupInput {
    pub GdiplusVersion: u32,
    pub DebugEventCallback: *mut c_void,
    pub SuppressBackgroundThread: BOOL,
    pub SuppressExternalCodecs: BOOL,
}

impl Default for GdiplusStartupInput {
    fn default() -> Self {
        GdiplusStartupInput {
            GdiplusVersion: 1,
            DebugEventCallback: std::ptr::null_mut(),
            SuppressBackgroundThread: 0,
            SuppressExternalCodecs: 0,
        }
    }
}

type GdiplusStartupFn =
    unsafe extern "system" fn(*mut usize, *const GdiplusStartupInput, *mut c_void) -> i32;
type GdiplusShutdownFn = unsafe extern "system" fn(usize);
type GdipCreateBitmapFromFileFn = unsafe extern "system" fn(*const u16, *mut *mut c_void) -> i32;
type GdipCreateHICONFromBitmapFn = unsafe extern "system" fn(*mut c_void, *mut HICON) -> i32;
type GdipDisposeImageFn = unsafe extern "system" fn(*mut c_void) -> i32;

/// `gdiplus.dll` loaded on demand, mirroring `Ole32` so the import table stays
/// `user32`+`kernel32`. Keeps the GDI+ token alive until dropped.
pub struct Gdiplus {
    module: HMODULE,
    token: usize,
    shutdown: GdiplusShutdownFn,
    create_bitmap_from_file: GdipCreateBitmapFromFileFn,
    create_hicon_from_bitmap: GdipCreateHICONFromBitmapFn,
    dispose_image: GdipDisposeImageFn,
}

impl Gdiplus {
    pub fn load() -> Option<Gdiplus> {
        unsafe {
            let module = LoadLibraryW(wide("gdiplus.dll").as_ptr());
            if module.is_null() {
                return None;
            }
            let startup = GetProcAddress(module, c"GdiplusStartup".as_ptr().cast::<u8>());
            let shutdown = GetProcAddress(module, c"GdiplusShutdown".as_ptr().cast::<u8>());
            let create_bitmap =
                GetProcAddress(module, c"GdipCreateBitmapFromFile".as_ptr().cast::<u8>());
            let create_hicon =
                GetProcAddress(module, c"GdipCreateHICONFromBitmap".as_ptr().cast::<u8>());
            let dispose = GetProcAddress(module, c"GdipDisposeImage".as_ptr().cast::<u8>());
            if startup.is_null()
                || shutdown.is_null()
                || create_bitmap.is_null()
                || create_hicon.is_null()
                || dispose.is_null()
            {
                FreeLibrary(module);
                return None;
            }

            let startup: GdiplusStartupFn = std::mem::transmute(startup);
            let shutdown: GdiplusShutdownFn = std::mem::transmute(shutdown);
            let create_bitmap_from_file: GdipCreateBitmapFromFileFn =
                std::mem::transmute(create_bitmap);
            let create_hicon_from_bitmap: GdipCreateHICONFromBitmapFn =
                std::mem::transmute(create_hicon);
            let dispose_image: GdipDisposeImageFn = std::mem::transmute(dispose);

            let input = GdiplusStartupInput::default();
            let mut token = 0usize;
            if startup(&mut token, &input, std::ptr::null_mut()) != 0 {
                FreeLibrary(module);
                return None;
            }
            Some(Gdiplus {
                module,
                token,
                shutdown,
                create_bitmap_from_file,
                create_hicon_from_bitmap,
                dispose_image,
            })
        }
    }

    /// Loads an icon file, trying the native `.ico` loader first and then GDI+
    /// for PNG/ICO/BMP. Returns `None` instead of failing the whole launcher.
    pub unsafe fn load_icon(&self, path: &Path) -> Option<HICON> {
        let name = wide(&path.to_string_lossy());
        let is_ico = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("ico"))
            .unwrap_or(false);
        if is_ico {
            let icon = LoadImageW(
                std::ptr::null_mut(),
                name.as_ptr(),
                IMAGE_ICON,
                0,
                0,
                LR_LOADFROMFILE,
            );
            if !icon.is_null() {
                return Some(icon as HICON);
            }
        }

        let mut bitmap: *mut c_void = std::ptr::null_mut();
        if (self.create_bitmap_from_file)(name.as_ptr(), &mut bitmap) != 0 || bitmap.is_null() {
            return None;
        }
        let mut icon: HICON = std::ptr::null_mut();
        let status = (self.create_hicon_from_bitmap)(bitmap, &mut icon);
        (self.dispose_image)(bitmap);
        (status == 0 && !icon.is_null()).then_some(icon)
    }
}

impl Drop for Gdiplus {
    fn drop(&mut self) {
        unsafe {
            (self.shutdown)(self.token);
            FreeLibrary(self.module);
        }
    }
}
