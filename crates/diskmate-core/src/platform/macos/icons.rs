#![allow(clippy::type_complexity)]
//! App icons, as Finder shows them.
//!
//! Asks `NSWorkspace` for the icon of a path (which handles every way an app
//! can carry its icon: an .icns file, an asset catalog, a document icon) and
//! draws it once into a square bitmap of the size asked for, then encodes that
//! as PNG. Read-only: nothing on disk is touched.

use std::ffi::{c_char, c_void, CString};
use std::path::Path;

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

type Id = *mut c_void;
type Sel = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// `NSCompositingOperationCopy`.
const COMPOSITE_COPY: usize = 1;
/// `NSBitmapImageFileTypePNG`.
const FILE_TYPE_PNG: usize = 4;

/// The icon for `path`, `size` pixels square, as PNG bytes.
pub fn icon_png(path: &Path, size: u32) -> Option<Vec<u8>> {
    let path = CString::new(path.to_str()?).ok()?;
    let side = f64::from(size.clamp(16, 1024));
    // SAFETY: Objective-C runtime calls inside an autorelease pool, each with
    // `objc_msgSend` cast to the exact signature of the message sent. Every
    // object used is checked for nil before it is messaged.
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let result = draw(&path, side);
        objc_autoreleasePoolPop(pool);
        result
    }
}

unsafe fn sel(name: &std::ffi::CStr) -> Sel {
    sel_registerName(name.as_ptr())
}

unsafe fn class(name: &std::ffi::CStr) -> Id {
    objc_getClass(name.as_ptr())
}

unsafe fn send0(target: Id, selector: Sel) -> Id {
    let f =
        std::mem::transmute::<unsafe extern "C" fn(), extern "C" fn(Id, Sel) -> Id>(objc_msgSend);
    f(target, selector)
}

unsafe fn send1(target: Id, selector: Sel, a: Id) -> Id {
    let f = std::mem::transmute::<unsafe extern "C" fn(), extern "C" fn(Id, Sel, Id) -> Id>(
        objc_msgSend,
    );
    f(target, selector, a)
}

unsafe fn draw(path: &CString, side: f64) -> Option<Vec<u8>> {
    let string = {
        let f = std::mem::transmute::<
            unsafe extern "C" fn(),
            extern "C" fn(Id, Sel, *const c_char) -> Id,
        >(objc_msgSend);
        f(
            class(c"NSString"),
            sel(c"stringWithUTF8String:"),
            path.as_ptr(),
        )
    };
    if string.is_null() {
        return None;
    }
    let workspace = send0(class(c"NSWorkspace"), sel(c"sharedWorkspace"));
    if workspace.is_null() {
        return None;
    }
    let image = send1(workspace, sel(c"iconForFile:"), string);
    if image.is_null() {
        return None;
    }

    // A 32-bit RGBA bitmap, `side` pixels square.
    let rep = send0(class(c"NSBitmapImageRep"), sel(c"alloc"));
    let rep = {
        let f = std::mem::transmute::<
            unsafe extern "C" fn(),
            extern "C" fn(
                Id,
                Sel,
                *mut c_void,
                isize,
                isize,
                isize,
                isize,
                i8,
                i8,
                Id,
                isize,
                isize,
            ) -> Id,
        >(objc_msgSend);
        let colour_space = send1(
            class(c"NSString"),
            sel(c"stringWithUTF8String:"),
            c"NSDeviceRGBColorSpace".as_ptr() as Id,
        );
        f(
            rep,
            sel(c"initWithBitmapDataPlanes:pixelsWide:pixelsHigh:bitsPerSample:samplesPerPixel:hasAlpha:isPlanar:colorSpaceName:bytesPerRow:bitsPerPixel:"),
            std::ptr::null_mut(),
            side as isize,
            side as isize,
            8,
            4,
            1,
            0,
            colour_space,
            0,
            0,
        )
    };
    if rep.is_null() {
        return None;
    }
    // Owned by the pool from here, like everything else.
    send0(rep, sel(c"autorelease"));

    let context = send1(
        class(c"NSGraphicsContext"),
        sel(c"graphicsContextWithBitmapImageRep:"),
        rep,
    );
    if context.is_null() {
        return None;
    }
    let context_class = class(c"NSGraphicsContext");
    send0(context_class, sel(c"saveGraphicsState"));
    send1(context_class, sel(c"setCurrentContext:"), context);
    {
        let f = std::mem::transmute::<
            unsafe extern "C" fn(),
            extern "C" fn(Id, Sel, Rect, Rect, usize, f64),
        >(objc_msgSend);
        let target = Rect {
            x: 0.0,
            y: 0.0,
            width: side,
            height: side,
        };
        let whole = Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
        f(
            image,
            sel(c"drawInRect:fromRect:operation:fraction:"),
            target,
            whole,
            COMPOSITE_COPY,
            1.0,
        );
    }
    send0(context_class, sel(c"restoreGraphicsState"));

    let properties = send0(class(c"NSDictionary"), sel(c"dictionary"));
    let data = {
        let f = std::mem::transmute::<
            unsafe extern "C" fn(),
            extern "C" fn(Id, Sel, usize, Id) -> Id,
        >(objc_msgSend);
        f(
            rep,
            sel(c"representationUsingType:properties:"),
            FILE_TYPE_PNG,
            properties,
        )
    };
    if data.is_null() {
        return None;
    }
    let length = {
        let f = std::mem::transmute::<unsafe extern "C" fn(), extern "C" fn(Id, Sel) -> usize>(
            objc_msgSend,
        );
        f(data, sel(c"length"))
    };
    let bytes = send0(data, sel(c"bytes")) as *const u8;
    if bytes.is_null() || length == 0 {
        return None;
    }
    Some(std::slice::from_raw_parts(bytes, length).to_vec())
}
