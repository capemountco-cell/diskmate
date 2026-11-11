//! Per-app memory, and the quit request, on macOS.
//!
//! Reading: `proc_listallpids`, `proc_pidpath` and `proc_pid_rusage` from
//! libproc, all documented. The figure used is `ri_phys_footprint`, the one
//! Activity Monitor shows in its Memory column. Processes owned by other users
//! refuse `proc_pid_rusage` without elevated rights; they are left out rather
//! than estimated.
//!
//! Quitting: `NSRunningApplication -terminate`, the same request the app's own
//! Quit menu item sends. The app may save, ask about unsaved changes, or
//! decline. It is only offered for apps with the regular activation policy,
//! meaning they have a Dock icon: never a background agent or daemon.
//!
//! AppKit is reached through the Objective-C runtime's C functions rather than
//! a bindings crate. Three messages do not justify a dependency.

use std::ffi::{c_char, c_int, c_void};

use crate::error::Result;
use crate::model::ProcessMemory;

// <libproc.h> and <sys/resource.h>. Linked from libSystem.
extern "C" {
    fn proc_listallpids(buffer: *mut c_void, buffersize: c_int) -> c_int;
    fn proc_pidpath(pid: c_int, buffer: *mut c_void, buffersize: u32) -> c_int;
    fn proc_pid_rusage(pid: c_int, flavor: c_int, buffer: *mut c_void) -> c_int;
}

const RUSAGE_INFO_V0: c_int = 0;
const PROC_PIDPATHINFO_MAXSIZE: usize = 4096;

/// `struct rusage_info_v0` from `<sys/resource.h>`.
#[repr(C)]
#[derive(Default)]
#[allow(dead_code)] // The layout must be complete; only the footprint is read.
struct RusageInfoV0 {
    ri_uuid: [u8; 16],
    ri_user_time: u64,
    ri_system_time: u64,
    ri_pkg_idle_wkups: u64,
    ri_interrupt_wkups: u64,
    ri_pageins: u64,
    ri_wired_size: u64,
    ri_resident_size: u64,
    ri_phys_footprint: u64,
    ri_proc_start_abstime: u64,
    ri_proc_exit_abstime: u64,
}

pub fn top_apps(limit: usize) -> Result<Vec<ProcessMemory>> {
    let mut rows = Vec::new();
    for pid in all_pids() {
        let Some(bytes) = footprint(pid) else {
            continue;
        };
        let path = pid_path(pid).unwrap_or_default();
        let name = path.rsplit('/').next().unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        rows.push((pid, path, name, bytes));
    }

    let mut apps = crate::platform::group_by_app(rows);
    apps.truncate(limit);
    for app in &mut apps {
        app.can_quit = app.bundle_path.is_some() && is_regular_app(app.pid);
    }
    Ok(apps)
}

fn all_pids() -> Vec<c_int> {
    // SAFETY: a null buffer asks for the number of pids.
    let count = unsafe { proc_listallpids(std::ptr::null_mut(), 0) };
    if count <= 0 {
        return Vec::new();
    }
    // Headroom for processes started between the two calls.
    let mut pids = vec![0 as c_int; count as usize + 64];
    let bytes = (pids.len() * std::mem::size_of::<c_int>()) as c_int;
    // SAFETY: the buffer holds `bytes` bytes; the call writes at most that.
    let filled = unsafe { proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
    if filled <= 0 {
        return Vec::new();
    }
    pids.truncate(filled as usize);
    pids.retain(|&pid| pid > 0);
    pids
}

fn footprint(pid: c_int) -> Option<i64> {
    let mut info = RusageInfoV0::default();
    // SAFETY: `info` matches the V0 layout the flavor requests.
    let rc =
        unsafe { proc_pid_rusage(pid, RUSAGE_INFO_V0, (&mut info as *mut RusageInfoV0).cast()) };
    (rc == 0).then_some(info.ri_phys_footprint as i64)
}

fn pid_path(pid: c_int) -> Option<String> {
    let mut buffer = vec![0u8; PROC_PIDPATHINFO_MAXSIZE];
    // SAFETY: the buffer is PROC_PIDPATHINFO_MAXSIZE bytes, as the call requires.
    let len = unsafe { proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    if len <= 0 {
        return None;
    }
    buffer.truncate(len as usize);
    String::from_utf8(buffer).ok()
}

// ------------------------------------------------------------ AppKit --

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_msgSend();
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

/// `NSApplicationActivationPolicyRegular`.
const ACTIVATION_POLICY_REGULAR: isize = 0;

/// Run `body` with the `NSRunningApplication` for `pid`, inside an autorelease
/// pool. `None` when there is no such application.
fn with_running_app<T>(pid: c_int, body: impl FnOnce(*mut c_void) -> T) -> Option<T> {
    // SAFETY: plain Objective-C runtime calls. `objc_msgSend` is cast to the
    // exact signature of each message sent, as the runtime requires.
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let class = objc_getClass(c"NSRunningApplication".as_ptr());
        let result = if class.is_null() {
            None
        } else {
            let lookup = std::mem::transmute::<
                unsafe extern "C" fn(),
                extern "C" fn(*mut c_void, *mut c_void, c_int) -> *mut c_void,
            >(objc_msgSend);
            let app = lookup(
                class,
                sel_registerName(c"runningApplicationWithProcessIdentifier:".as_ptr()),
                pid,
            );
            (!app.is_null()).then(|| body(app))
        };
        objc_autoreleasePoolPop(pool);
        result
    }
}

fn activation_policy(app: *mut c_void) -> isize {
    // SAFETY: `-activationPolicy` takes no arguments and returns an NSInteger.
    unsafe {
        let send = std::mem::transmute::<
            unsafe extern "C" fn(),
            extern "C" fn(*mut c_void, *mut c_void) -> isize,
        >(objc_msgSend);
        send(app, sel_registerName(c"activationPolicy".as_ptr()))
    }
}

fn is_regular_app(pid: c_int) -> bool {
    with_running_app(pid, activation_policy) == Some(ACTIVATION_POLICY_REGULAR)
}

/// Ask an ordinary app to quit, as its Quit menu item would.
pub fn request_quit(pid: i32) -> std::result::Result<(), String> {
    let outcome = with_running_app(pid, |app| {
        if activation_policy(app) != ACTIVATION_POLICY_REGULAR {
            return Err("only apps with a Dock icon can be asked to quit".to_string());
        }
        // SAFETY: `-terminate` takes no arguments and returns a BOOL, one byte
        // on both Apple architectures.
        let delivered = unsafe {
            let send = std::mem::transmute::<
                unsafe extern "C" fn(),
                extern "C" fn(*mut c_void, *mut c_void) -> i8,
            >(objc_msgSend);
            send(app, sel_registerName(c"terminate".as_ptr()))
        };
        if delivered != 0 {
            Ok(())
        } else {
            Err("the app did not accept the quit request".to_string())
        }
    });
    outcome.unwrap_or_else(|| Err("that app is no longer running".to_string()))
}
