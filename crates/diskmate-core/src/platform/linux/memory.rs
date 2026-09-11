//! Memory snapshot from `/proc/meminfo`.

use crate::error::{CoreError, Result};
use crate::model::{MemoryPressure, MemorySnapshot};

pub fn snapshot() -> Result<MemorySnapshot> {
    let text =
        std::fs::read_to_string("/proc/meminfo").map_err(|e| CoreError::io("/proc/meminfo", e))?;
    Ok(parse_meminfo(&text))
}

pub(crate) fn parse_meminfo(text: &str) -> MemorySnapshot {
    let field = |name: &str| -> Option<i64> {
        text.lines()
            .find(|line| line.starts_with(name) && line.as_bytes().get(name.len()) == Some(&b':'))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<i64>().ok())
            .map(|kb| kb * 1024)
    };

    let total = field("MemTotal").unwrap_or(0);
    let available = field("MemAvailable").unwrap_or(0);
    let free = field("MemFree").unwrap_or(0);
    let cached = field("Cached").unwrap_or(0) + field("Buffers").unwrap_or(0);
    let swap_total = field("SwapTotal");
    let swap_free = field("SwapFree");

    // "Used" excludes reclaimable cache. Reporting cache as used is the mistake
    // that makes people think a healthy machine is out of memory.
    let used = (total - available).max(0);

    MemorySnapshot {
        total_bytes: total,
        used_bytes: used,
        cached_bytes: cached,
        free_bytes: free,
        wired_bytes: None,
        compressed_bytes: None,
        swap_used_bytes: match (swap_total, swap_free) {
            (Some(total), Some(free)) => Some((total - free).max(0)),
            _ => None,
        },
        swap_total_bytes: swap_total,
        pressure: pressure_from_availability(total, available),
        sampled_at: crate::db::now_unix(),
    }
}

/// Linux has no direct equivalent of macOS's pressure level, so this is derived
/// from how much memory the kernel says it can still hand out. The thresholds
/// are conservative and the value is only ever used to phrase an explanation,
/// never to justify a "clean up now" prompt.
fn pressure_from_availability(total: i64, available: i64) -> MemoryPressure {
    if total <= 0 {
        return MemoryPressure::Unknown;
    }
    let ratio = available as f64 / total as f64;
    if ratio > 0.20 {
        MemoryPressure::Normal
    } else if ratio > 0.08 {
        MemoryPressure::Warning
    } else {
        MemoryPressure::Critical
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "MemTotal:       16316476 kB\n\
                          MemFree:         1234567 kB\n\
                          MemAvailable:    8158238 kB\n\
                          Buffers:          100000 kB\n\
                          Cached:          4000000 kB\n\
                          SwapTotal:       2097148 kB\n\
                          SwapFree:        2097148 kB\n";

    #[test]
    fn meminfo_is_parsed_into_bytes() {
        let snapshot = parse_meminfo(SAMPLE);
        assert_eq!(snapshot.total_bytes, 16_316_476 * 1024);
        assert_eq!(snapshot.cached_bytes, 4_100_000 * 1024);
        assert_eq!(snapshot.swap_used_bytes, Some(0));
    }

    #[test]
    fn cache_is_not_counted_as_used() {
        let snapshot = parse_meminfo(SAMPLE);
        assert_eq!(snapshot.used_bytes, (16_316_476 - 8_158_238) * 1024);
        assert_eq!(snapshot.pressure, MemoryPressure::Normal);
    }

    #[test]
    fn a_field_prefix_does_not_match_a_longer_name() {
        // "MemTotal" must not be satisfied by "MemTotalFoo".
        let snapshot =
            parse_meminfo("MemTotalFoo: 1 kB\nMemTotal: 2048 kB\nMemAvailable: 1024 kB\n");
        assert_eq!(snapshot.total_bytes, 2048 * 1024);
    }
}
