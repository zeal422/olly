#[cfg(windows)]
pub(crate) fn maximize_console_window() {
    #[link(name = "Kernel32")]
    extern "system" {
        fn GetConsoleWindow() -> *mut core::ffi::c_void;
    }

    #[link(name = "User32")]
    extern "system" {
        fn ShowWindow(hwnd: *mut core::ffi::c_void, command: i32) -> i32;
    }

    const SW_MAXIMIZE: i32 = 3;
    // SAFETY: We only call Win32 API on this process console window handle.
    // If no console exists, hwnd is null and this function becomes a no-op.
    unsafe {
        let hwnd = GetConsoleWindow();
        if !hwnd.is_null() {
            let _ = ShowWindow(hwnd, SW_MAXIMIZE);
        }
    }
}

#[cfg(windows)]
pub(crate) fn hide_console_scrollbars() {
    #[link(name = "Kernel32")]
    extern "system" {
        fn GetConsoleWindow() -> *mut core::ffi::c_void;
    }

    #[link(name = "User32")]
    extern "system" {
        fn ShowScrollBar(hwnd: *mut core::ffi::c_void, w_bar: i32, b_show: i32) -> i32;
    }

    const SB_BOTH: i32 = 3;

    // SAFETY: This only targets the current process console window. Null handles are ignored.
    unsafe {
        let hwnd = GetConsoleWindow();
        if !hwnd.is_null() {
            let _ = ShowScrollBar(hwnd, SB_BOTH, 0);
        }
    }
}

#[cfg(windows)]
pub(crate) fn snap_console_window_to_cell_grid() {
    #[repr(C)]
    struct Coord {
        x: i16,
        y: i16,
    }

    #[repr(C)]
    struct ConsoleFontInfoEx {
        cb_size: u32,
        n_font: u32,
        dw_font_size: Coord,
        font_family: u32,
        font_weight: u32,
        face_name: [u16; 32],
    }

    #[repr(C)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[link(name = "Kernel32")]
    extern "system" {
        fn GetConsoleWindow() -> *mut core::ffi::c_void;
        fn GetStdHandle(std_handle: i32) -> *mut core::ffi::c_void;
        fn GetCurrentConsoleFontEx(
            console_output: *mut core::ffi::c_void,
            maximum_window: i32,
            console_current_font_ex: *mut ConsoleFontInfoEx,
        ) -> i32;
    }

    #[link(name = "User32")]
    extern "system" {
        fn GetClientRect(hwnd: *mut core::ffi::c_void, rect: *mut WinRect) -> i32;
        fn GetWindowRect(hwnd: *mut core::ffi::c_void, rect: *mut WinRect) -> i32;
        fn SetWindowPos(
            hwnd: *mut core::ffi::c_void,
            hwnd_insert_after: *mut core::ffi::c_void,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }

    const STD_OUTPUT_HANDLE: i32 = -11;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;

    // SAFETY: Uses current process console window/handle and clamps values to valid ranges.
    unsafe {
        let hwnd = GetConsoleWindow();
        if hwnd.is_null() {
            return;
        }

        let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if stdout_handle.is_null() || (stdout_handle as isize) == -1 {
            return;
        }

        let mut font = ConsoleFontInfoEx {
            cb_size: std::mem::size_of::<ConsoleFontInfoEx>() as u32,
            n_font: 0,
            dw_font_size: Coord { x: 0, y: 0 },
            font_family: 0,
            font_weight: 0,
            face_name: [0; 32],
        };
        if GetCurrentConsoleFontEx(stdout_handle, 0, &mut font as *mut ConsoleFontInfoEx) == 0 {
            return;
        }
        let font_w = i32::from(font.dw_font_size.x.max(1));
        let font_h = i32::from(font.dw_font_size.y.max(1));

        let mut client_rect = WinRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let mut window_rect = WinRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetClientRect(hwnd, &mut client_rect as *mut WinRect) == 0
            || GetWindowRect(hwnd, &mut window_rect as *mut WinRect) == 0
        {
            return;
        }

        let client_w = (client_rect.right - client_rect.left).max(1);
        let client_h = (client_rect.bottom - client_rect.top).max(1);
        let window_w = (window_rect.right - window_rect.left).max(1);
        let window_h = (window_rect.bottom - window_rect.top).max(1);
        let non_client_w = window_w.saturating_sub(client_w);
        let non_client_h = window_h.saturating_sub(client_h);

        let snapped_client_w = ((client_w / font_w).max(1)).saturating_mul(font_w);
        let snapped_client_h = ((client_h / font_h).max(1)).saturating_mul(font_h);
        let target_w = snapped_client_w.saturating_add(non_client_w).max(100);
        let target_h = snapped_client_h.saturating_add(non_client_h).max(100);

        if target_w != window_w || target_h != window_h {
            let _ = SetWindowPos(
                hwnd,
                core::ptr::null_mut(),
                0,
                0,
                target_w,
                target_h,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

#[cfg(windows)]
pub(crate) fn sync_console_buffer_to_window(normalize_origin: bool) {
    #[repr(C)]
    struct Coord {
        x: i16,
        y: i16,
    }

    #[repr(C)]
    struct SmallRect {
        left: i16,
        top: i16,
        right: i16,
        bottom: i16,
    }

    #[repr(C)]
    struct ConsoleScreenBufferInfo {
        dw_size: Coord,
        dw_cursor_position: Coord,
        w_attributes: u16,
        sr_window: SmallRect,
        dw_maximum_window_size: Coord,
    }

    #[link(name = "Kernel32")]
    extern "system" {
        fn GetStdHandle(std_handle: i32) -> *mut core::ffi::c_void;
        fn GetConsoleScreenBufferInfo(
            console_output: *mut core::ffi::c_void,
            console_screen_buffer_info: *mut ConsoleScreenBufferInfo,
        ) -> i32;
        fn SetConsoleWindowInfo(
            console_output: *mut core::ffi::c_void,
            absolute: i32,
            console_window: *const SmallRect,
        ) -> i32;
        fn SetConsoleScreenBufferSize(console_output: *mut core::ffi::c_void, size: Coord) -> i32;
    }

    const STD_OUTPUT_HANDLE: i32 = -11;

    // SAFETY: Uses this process console output handle and validates dimensions before applying.
    unsafe {
        let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if stdout_handle.is_null() || (stdout_handle as isize) == -1 {
            return;
        }
        let mut info = std::mem::MaybeUninit::<ConsoleScreenBufferInfo>::uninit();
        if GetConsoleScreenBufferInfo(stdout_handle, info.as_mut_ptr()) == 0 {
            return;
        }
        let info = info.assume_init();
        let window = info.sr_window;
        let width = window.right - window.left + 1;
        let height = window.bottom - window.top + 1;
        if width <= 0 || height <= 0 {
            return;
        }

        // Ensure buffer is never smaller than viewport before normalization.
        let needed_w = info.dw_size.x.max(width);
        let needed_h = info.dw_size.y.max(height);
        if needed_w != info.dw_size.x || needed_h != info.dw_size.y {
            let _ = SetConsoleScreenBufferSize(
                stdout_handle,
                Coord {
                    x: needed_w,
                    y: needed_h,
                },
            );
        }

        if normalize_origin && (window.left != 0 || window.top != 0) {
            let normalized_window = SmallRect {
                left: 0,
                top: 0,
                right: width - 1,
                bottom: height - 1,
            };
            let _ = SetConsoleWindowInfo(stdout_handle, 1, &normalized_window);
        }

        // Lock buffer to viewport to prevent native scrollbars in alternate screen.
        if info.dw_size.x != width || info.dw_size.y != height {
            let _ = SetConsoleScreenBufferSize(
                stdout_handle,
                Coord {
                    x: width,
                    y: height,
                },
            );
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn maximize_console_window() {}

#[cfg(not(windows))]
pub(crate) fn hide_console_scrollbars() {}

#[cfg(not(windows))]
pub(crate) fn snap_console_window_to_cell_grid() {}

#[cfg(not(windows))]
pub(crate) fn sync_console_buffer_to_window(_normalize_origin: bool) {}
