//! # `binsize::utils`
//!
//! Everything else that didn't fit into any of the other modules
//!

use std::io;
use std::mem;

const DEFAULT_MAX_TERM_COLS: usize = 80;

/// Represents soring order
#[derive(Copy, Clone)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Unix (Linux/Mac) version of `terminal_size` - returns `(cols, rows)` if available
#[cfg(unix)]
pub fn terminal_size() -> io::Result<(u16, u16)> {
    use std::os::unix::io::AsRawFd;

    #[repr(C)]
    struct WinSize {
        ws_row: u16,
        ws_col: u16,
        ws_xpixel: u16,
        ws_ypixel: u16,
    }

    let fd = std::io::stdout().as_raw_fd();
    let mut ws: WinSize = unsafe { mem::zeroed() };
    let result = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) };

    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok((ws.ws_col, ws.ws_row))
    }
}

/// Windows version of `terminal_size` - returns `(cols, rows)` if available
#[cfg(windows)]
pub fn terminal_size() -> io::Result<(u16, u16)> {
    use winapi::um::wincon::{GetConsoleScreenBufferInfo, CONSOLE_SCREEN_BUFFER_INFO};
    use winapi::um::processenv::GetStdHandle;
    use winapi::um::winbase::STD_OUTPUT_HANDLE;

    unsafe {
        let h = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = mem::zeroed();

        if GetConsoleScreenBufferInfo(h, &mut csbi) == 0 {
            Err(io::Error::last_os_error())
        } else {
            let cols = (csbi.srWindow.Right - csbi.srWindow.Left + 1) as u16;
            let rows = (csbi.srWindow.Bottom - csbi.srWindow.Top + 1) as u16;
            Ok((cols, rows))
        }
    }
}


/// Shortcut to `terminal_size().cols`, if available, otherwise returns default max cols
pub fn term_width() -> usize {
    match terminal_size() {
        Ok((cols, _)) => (cols - 1) as usize,
        Err(_)        => DEFAULT_MAX_TERM_COLS,
    }
}

