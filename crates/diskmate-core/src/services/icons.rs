//! App icons for the interface.
//!
//! The interface asks for the icon of an app by its path and gets back a PNG
//! as a `data:` URL it can put straight into an image. Icons are drawn once
//! per path and size and kept in memory, so scrolling a list of apps does not
//! ask macOS for the same icon twice. Only paths that end in `.app` are
//! answered: this is for app icons, not a way to read arbitrary files.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::platform::{self, SystemProbe};

/// The largest icon the interface may ask for, in pixels.
const MAX_SIZE: u32 = 256;
/// Enough for every app on a well-stocked Mac, a few times over.
const MAX_CACHED: usize = 2_000;

pub struct IconService {
    probe: &'static dyn SystemProbe,
    cache: Mutex<HashMap<(String, u32), Option<String>>>,
}

impl Default for IconService {
    fn default() -> Self {
        Self::new()
    }
}

impl IconService {
    pub fn new() -> Self {
        Self::with_probe(platform::probe())
    }

    pub fn with_probe(probe: &'static dyn SystemProbe) -> Self {
        Self {
            probe,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// The icon for the app at `path` as a `data:image/png;base64,…` URL, or
    /// `None` when it is not an app or has no icon that can be read.
    pub fn app_icon(&self, path: &str, size: u32) -> Option<String> {
        let trimmed = path.trim_end_matches('/');
        if !trimmed.ends_with(".app") || !Path::new(trimmed).is_absolute() {
            return None;
        }
        let size = size.clamp(16, MAX_SIZE);
        let key = (trimmed.to_string(), size);
        if let Some(found) = self.cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
            return found;
        }
        let url = self
            .probe
            .icon_png(Path::new(trimmed), size)
            .map(|png| format!("data:image/png;base64,{}", base64(&png)));
        if let Ok(mut cache) = self.cache.lock() {
            if cache.len() >= MAX_CACHED {
                cache.clear();
            }
            cache.insert(key, url.clone());
        }
        url
    }
}

/// Standard base64 with padding. Twenty lines rather than a dependency.
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[0xff, 0xfe, 0xfd]), "//79");
    }

    #[test]
    fn only_app_paths_are_answered() {
        let service = IconService::new();
        assert_eq!(service.app_icon("/etc/passwd", 64), None);
        assert_eq!(service.app_icon("relative/Thing.app", 64), None);
        // Off macOS there are no icons; on macOS this app does not exist.
        assert_eq!(
            service.app_icon("/Applications/No Such App 12345.app", 64),
            None
        );
    }
}
