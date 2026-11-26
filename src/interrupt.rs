//! SIGINT handling for the binary, on Unix only. The handler is the one place
//! besides `stdout_probe` that needs `unsafe`, because the C library has no safe
//! binding in the standard library.
//!
//! Before the report is written, SIGINT writes one line to standard error and
//! ends the process with status 130 at once; nothing was written to standard
//! output yet. While the report is being written, SIGINT is only noted, the
//! write finishes (one whole report, never half of one), and then the process
//! ends with 130 as well.

#[cfg(unix)]
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::{AtomicBool, Ordering};

pub const EXIT_SIGINT: u8 = 130;
pub const MESSAGE: &str = "ebs-orphan-bill: interrupted by SIGINT\n";

static WRITING: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicBool = AtomicBool::new(false);
/// NUL-terminated path of a temporary `--output` file that SIGINT must remove.
#[cfg(unix)]
static CLEANUP: AtomicPtr<std::ffi::c_char> = AtomicPtr::new(std::ptr::null_mut());

#[cfg(unix)]
mod sys {
    use super::{Ordering, CLEANUP, EXIT_SIGINT, MESSAGE, PENDING, WRITING};

    extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
        #[link_name = "write"]
        fn write_fd(fd: i32, buf: *const std::ffi::c_void, count: usize) -> isize;
        fn unlink(path: *const std::ffi::c_char) -> i32;
        fn _exit(status: i32) -> !;
    }
    const SIGINT: i32 = 2;

    extern "C" fn on_sigint(_signum: i32) {
        if WRITING.load(Ordering::SeqCst) {
            PENDING.store(true, Ordering::SeqCst);
            return;
        }
        // SAFETY: `unlink`, `write` and `_exit` are async-signal-safe; the buffer
        // is a `'static` string, the cleanup path (when set) is a leaked
        // NUL-terminated string, and `_exit` does not run any destructor.
        unsafe {
            let path = CLEANUP.load(Ordering::SeqCst);
            if !path.is_null() {
                unlink(path);
            }
            write_fd(2, MESSAGE.as_ptr().cast(), MESSAGE.len());
            _exit(i32::from(EXIT_SIGINT));
        }
    }

    pub fn install() {
        // SAFETY: `signal` registers a handler that only uses async-signal-safe
        // calls and atomics.
        unsafe {
            signal(SIGINT, on_sigint as extern "C" fn(i32) as usize);
        }
    }
}

/// Installs the SIGINT handler. Does nothing on a platform without signals.
pub fn install() {
    #[cfg(unix)]
    sys::install();
}

/// Marks the start (`true`) or the end (`false`) of the write of the report.
pub fn set_writing(writing: bool) {
    WRITING.store(writing, Ordering::SeqCst);
}

/// Was SIGINT caught while the report was being written?
pub fn pending() -> bool {
    PENDING.load(Ordering::SeqCst)
}

/// Names the temporary file that a SIGINT must remove (`Some`), or says there is
/// none any more (`None`). The path is leaked on purpose: a handler cannot free.
pub fn set_cleanup(path: Option<&std::path::Path>) {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let ptr = match path.and_then(|p| std::ffi::CString::new(p.as_os_str().as_bytes()).ok()) {
            Some(text) => text.into_raw(),
            None => std::ptr::null_mut(),
        };
        CLEANUP.store(ptr, Ordering::SeqCst);
    }
    #[cfg(not(unix))]
    let _ = path;
}
