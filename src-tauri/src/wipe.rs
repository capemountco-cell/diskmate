//! Wipe-down mode: the keyboard and trackpad ignore everything for a short,
//! fixed time, so the Mac can be cleaned without typing or clicking anything.
//!
//! How it stays safe:
//!
//! - It always ends on its own. The timer is capped at three minutes, and the
//!   thread that holds the lock checks the deadline itself, four times a
//!   second, independently of the window and of the thread that reports
//!   progress. Nothing the interface does (or fails to do) can keep it on.
//! - Holding Esc and Return together for three seconds ends it early. The
//!   keys are watched by the same tap that swallows them.
//! - It needs Accessibility permission, which is how macOS lets any app pause
//!   the keyboard. Mr Clean only asks when wipe-down mode is first started,
//!   and uses the permission for nothing else: the tap exists only while the
//!   timer runs.
//! - It reads no key other than Esc and Return, and stores nothing.
//!
//! The window goes full screen and black while it runs, because smudges show
//! best on black.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

/// The longest wipe-down there can be, whatever the interface asks for.
pub const MAX_SECONDS: u32 = 180;
const MIN_SECONDS: u32 = 10;
/// How long Esc and Return must be held together to stop early.
const HOLD: Duration = Duration::from_secs(3);
/// Returned when Accessibility permission has not been given yet, so the
/// interface can explain it instead of showing an error.
pub const NEEDS_PERMISSION: &str = "accessibility";

static ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Started {
    seconds: u32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Held {
    /// 0 to 1: how far through the three-second hold the person is.
    fraction: f64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Ended {
    /// "timer" or "held".
    reason: &'static str,
}

/// Whether Mr Clean may pause the keyboard and trackpad. Asks nothing.
#[tauri::command]
pub fn wipe_permission() -> bool {
    platform::trusted(false)
}

/// Asks macOS for Accessibility permission, which shows its own prompt and
/// lists Mr Clean in System Settings. Returns whether it is already granted.
#[tauri::command]
pub fn wipe_request_permission() -> bool {
    platform::trusted(true)
}

/// Locks the keyboard and trackpad for `seconds` (10 to 180).
#[tauri::command]
pub fn wipe_start<R: Runtime>(app: AppHandle<R>, seconds: u32) -> Result<(), String> {
    if ACTIVE.swap(true, Ordering::SeqCst) {
        return Err("Wipe-down mode is already running.".into());
    }
    if !platform::trusted(false) {
        ACTIVE.store(false, Ordering::SeqCst);
        return Err(NEEDS_PERMISSION.into());
    }
    let seconds = seconds.clamp(MIN_SECONDS, MAX_SECONDS);
    let deadline = Instant::now() + Duration::from_secs(u64::from(seconds));

    if let Err(error) = platform::lock(deadline) {
        ACTIVE.store(false, Ordering::SeqCst);
        return Err(error);
    }

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.set_always_on_top(true);
        let _ = window.set_simple_fullscreen(true);
        let _ = window.set_cursor_visible(false);
    }
    let _ = app.emit("wipe:started", Started { seconds });

    // Reports the hold and decides when to stop. The lock thread checks the
    // deadline on its own as well, so this thread is not what keeps it safe.
    std::thread::Builder::new()
        .name("mrclean-wipe-watch".into())
        .spawn(move || {
            let mut held_since: Option<Instant> = None;
            let reason = loop {
                std::thread::sleep(Duration::from_millis(100));
                if Instant::now() >= deadline || !platform::running() {
                    break "timer";
                }
                if platform::stop_keys_down() {
                    let since = *held_since.get_or_insert_with(Instant::now);
                    let fraction = since.elapsed().as_secs_f64() / HOLD.as_secs_f64();
                    let _ = app.emit(
                        "wipe:held",
                        Held {
                            fraction: fraction.min(1.0),
                        },
                    );
                    if fraction >= 1.0 {
                        break "held";
                    }
                } else if held_since.take().is_some() {
                    let _ = app.emit("wipe:held", Held { fraction: 0.0 });
                }
            };
            platform::unlock();
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_simple_fullscreen(false);
                let _ = window.set_always_on_top(false);
                let _ = window.set_cursor_visible(true);
            }
            ACTIVE.store(false, Ordering::SeqCst);
            let _ = app.emit("wipe:ended", Ended { reason });
        })
        .map_err(|error| error.to_string())?;

    Ok(())
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::c_void;
    use std::ptr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    type CFTypeRef = *const c_void;
    type CFStringRef = *const c_void;
    type CFDictionaryRef = *const c_void;
    type CFAllocatorRef = *const c_void;
    type CFMachPortRef = *mut c_void;
    type CFRunLoopSourceRef = *mut c_void;
    type CFRunLoopRef = *mut c_void;
    type CGEventRef = *mut c_void;
    type CGEventTapProxy = *mut c_void;
    type TapCallback = extern "C" fn(CGEventTapProxy, u32, CGEventRef, *mut c_void) -> CGEventRef;

    /// Opaque: only its address is passed.
    #[repr(C)]
    struct CFDictionaryCallBacks {
        _private: [u8; 0],
    }

    #[allow(non_upper_case_globals)]
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: u64,
            callback: TapCallback,
            user_info: *mut c_void,
        ) -> CFMachPortRef;
        fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
        fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
        static kAXTrustedCheckOptionPrompt: CFStringRef;
    }

    #[allow(non_upper_case_globals)]
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFMachPortCreateRunLoopSource(
            allocator: CFAllocatorRef,
            port: CFMachPortRef,
            order: isize,
        ) -> CFRunLoopSourceRef;
        fn CFRunLoopGetCurrent() -> CFRunLoopRef;
        fn CFRunLoopAddSource(
            run_loop: CFRunLoopRef,
            source: CFRunLoopSourceRef,
            mode: CFStringRef,
        );
        fn CFRunLoopRunInMode(mode: CFStringRef, seconds: f64, return_after_source: u8) -> i32;
        fn CFRelease(object: CFTypeRef);
        fn CFDictionaryCreate(
            allocator: CFAllocatorRef,
            keys: *const *const c_void,
            values: *const *const c_void,
            count: isize,
            key_callbacks: *const CFDictionaryCallBacks,
            value_callbacks: *const CFDictionaryCallBacks,
        ) -> CFDictionaryRef;
        static kCFRunLoopCommonModes: CFStringRef;
        static kCFRunLoopDefaultMode: CFStringRef;
        static kCFBooleanTrue: CFTypeRef;
        static kCFBooleanFalse: CFTypeRef;
        static kCFTypeDictionaryKeyCallBacks: CFDictionaryCallBacks;
        static kCFTypeDictionaryValueCallBacks: CFDictionaryCallBacks;
    }

    const SESSION_TAP: u32 = 1;
    const HEAD_INSERT: u32 = 0;
    const DEFAULT_OPTIONS: u32 = 0;
    const ALL_EVENTS: u64 = u64::MAX;
    const KEY_DOWN: u32 = 10;
    const KEY_UP: u32 = 11;
    const KEYCODE_FIELD: u32 = 9;
    const ESCAPE: i64 = 53;
    const RETURN: i64 = 36;
    const KEYPAD_ENTER: i64 = 76;
    const DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
    const DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
    /// A little past the deadline: the watcher should have stopped it first.
    const GRACE: Duration = Duration::from_secs(2);

    static RUNNING: AtomicBool = AtomicBool::new(false);
    static STOP: AtomicBool = AtomicBool::new(false);
    static ESCAPE_DOWN: AtomicBool = AtomicBool::new(false);
    static RETURN_DOWN: AtomicBool = AtomicBool::new(false);
    /// The tap, so the callback can switch it back on if macOS pauses it.
    static TAP: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    pub fn trusted(prompt: bool) -> bool {
        // SAFETY: a one-entry CFDictionary of CoreFoundation constants, passed
        // to AX and released after.
        unsafe {
            let keys = [kAXTrustedCheckOptionPrompt];
            let values = [if prompt {
                kCFBooleanTrue
            } else {
                kCFBooleanFalse
            }];
            let options = CFDictionaryCreate(
                ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                1,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );
            let trusted = AXIsProcessTrustedWithOptions(options);
            if !options.is_null() {
                CFRelease(options);
            }
            trusted
        }
    }

    pub fn running() -> bool {
        RUNNING.load(Ordering::SeqCst)
    }

    pub fn stop_keys_down() -> bool {
        ESCAPE_DOWN.load(Ordering::SeqCst) && RETURN_DOWN.load(Ordering::SeqCst)
    }

    pub fn unlock() {
        STOP.store(true, Ordering::SeqCst);
    }

    /// Swallows every event, noting only whether Esc and Return are down.
    extern "C" fn swallow(
        _proxy: CGEventTapProxy,
        kind: u32,
        event: CGEventRef,
        _user: *mut c_void,
    ) -> CGEventRef {
        if kind == DISABLED_BY_TIMEOUT || kind == DISABLED_BY_USER_INPUT {
            let tap = TAP.load(Ordering::SeqCst) as CFMachPortRef;
            if !tap.is_null() && !STOP.load(Ordering::SeqCst) {
                // SAFETY: the tap stays alive until the lock thread releases
                // it, after which TAP is zero.
                unsafe { CGEventTapEnable(tap, true) };
            }
            return event;
        }
        if kind == KEY_DOWN || kind == KEY_UP {
            // SAFETY: `event` is the live event macOS passed in.
            let code = unsafe { CGEventGetIntegerValueField(event, KEYCODE_FIELD) };
            let down = kind == KEY_DOWN;
            match code {
                ESCAPE => ESCAPE_DOWN.store(down, Ordering::SeqCst),
                RETURN | KEYPAD_ENTER => RETURN_DOWN.store(down, Ordering::SeqCst),
                _ => {}
            }
        }
        ptr::null_mut()
    }

    pub fn lock(deadline: Instant) -> Result<(), String> {
        STOP.store(false, Ordering::SeqCst);
        ESCAPE_DOWN.store(false, Ordering::SeqCst);
        RETURN_DOWN.store(false, Ordering::SeqCst);
        let (ready, started) = mpsc::channel::<Result<(), String>>();

        std::thread::Builder::new()
            .name("mrclean-wipe-lock".into())
            .spawn(move || {
                // SAFETY: the tap and its run-loop source are created, used and
                // released on this thread only.
                unsafe {
                    let tap = CGEventTapCreate(
                        SESSION_TAP,
                        HEAD_INSERT,
                        DEFAULT_OPTIONS,
                        ALL_EVENTS,
                        swallow,
                        ptr::null_mut(),
                    );
                    if tap.is_null() {
                        let _ = ready.send(Err(
                            "macOS did not let Mr Clean pause the keyboard. Check that it is allowed under Privacy & Security › Accessibility.".into(),
                        ));
                        return;
                    }
                    TAP.store(tap as usize, Ordering::SeqCst);
                    let source = CFMachPortCreateRunLoopSource(ptr::null(), tap, 0);
                    CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
                    CGEventTapEnable(tap, true);
                    RUNNING.store(true, Ordering::SeqCst);
                    let _ = ready.send(Ok(()));

                    // Run in short slices and check both the stop flag and the
                    // deadline every time, so nothing can leave the lock on.
                    while !STOP.load(Ordering::SeqCst) && Instant::now() < deadline + GRACE {
                        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.25, 0);
                    }

                    CGEventTapEnable(tap, false);
                    TAP.store(0, Ordering::SeqCst);
                    RUNNING.store(false, Ordering::SeqCst);
                    if !source.is_null() {
                        CFRelease(source);
                    }
                    CFRelease(tap);
                }
            })
            .map_err(|error| error.to_string())?;

        started
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "The keyboard lock did not start.".to_string())?
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use std::time::Instant;

    pub fn trusted(_prompt: bool) -> bool {
        false
    }
    pub fn running() -> bool {
        false
    }
    pub fn stop_keys_down() -> bool {
        false
    }
    pub fn unlock() {}
    pub fn lock(_deadline: Instant) -> Result<(), String> {
        Err("Wipe-down mode needs macOS.".into())
    }
}
