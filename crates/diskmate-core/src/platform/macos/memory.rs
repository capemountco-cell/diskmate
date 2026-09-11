//! Memory statistics on macOS.
//!
//! Three documented sources, no private interfaces:
//!
//! * `sysctl hw.memsize` for installed RAM.
//! * `host_statistics64(HOST_VM_INFO64)` for the page-level breakdown.
//! * `sysctl vm.swapusage` for swap.
//!
//! What DiskMate reports as "used" is anonymous plus wired plus compressed
//! memory. The file-backed and purgeable pages macOS is holding are reported
//! separately as cached, because they are available on demand. A Mac with
//! almost no "free" memory and normal pressure is a Mac working correctly, and
//! DiskMate says so instead of offering to fix it.

use crate::error::{CoreError, Result};
use crate::model::{MemoryPressure, MemorySnapshot};

pub fn snapshot() -> Result<MemorySnapshot> {
    let page_size = page_size();
    let total = sysctl_u64(b"hw.memsize\0")
        .ok_or_else(|| CoreError::invalid("hw.memsize is unavailable"))? as i64;
    let vm = vm_statistics()?;

    let pages = |count: u64| -> i64 { count as i64 * page_size };

    let wired = pages(vm.wire_count as u64);
    let compressed = pages(vm.compressor_page_count as u64);
    let anonymous = pages(vm.internal_page_count.saturating_sub(vm.purgeable_count) as u64);
    let cached = pages((vm.external_page_count + vm.purgeable_count) as u64);
    let free = pages(vm.free_count.saturating_sub(vm.speculative_count) as u64);

    let (swap_total, swap_used) = swap_usage();

    Ok(MemorySnapshot {
        total_bytes: total,
        used_bytes: anonymous + wired + compressed,
        cached_bytes: cached,
        free_bytes: free,
        wired_bytes: Some(wired),
        compressed_bytes: Some(compressed),
        swap_used_bytes: swap_used,
        swap_total_bytes: swap_total,
        pressure: pressure_level(),
        sampled_at: crate::db::now_unix(),
    })
}

fn page_size() -> i64 {
    // SAFETY: `sysconf` takes an integer name and returns a long.
    let value = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if value > 0 {
        value as i64
    } else {
        4096
    }
}

/// `vm_statistics64_data_t` from `<mach/vm_statistics.h>`.
///
/// Declared locally because the `libc` crate does not expose the Mach VM
/// structures for every Apple target, and the layout is a stable part of the
/// kernel ABI.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct VmStatistics64 {
    free_count: u32,
    active_count: u32,
    inactive_count: u32,
    wire_count: u32,
    zero_fill_count: u64,
    reactivations: u64,
    pageins: u64,
    pageouts: u64,
    faults: u64,
    cow_faults: u64,
    lookups: u64,
    hits: u64,
    purges: u64,
    purgeable_count: u32,
    speculative_count: u32,
    decompressions: u64,
    compressions: u64,
    swapins: u64,
    swapouts: u64,
    compressor_page_count: u32,
    throttled_count: u32,
    external_page_count: u32,
    internal_page_count: u32,
    total_uncompressed_pages_in_compressor: u64,
}

const HOST_VM_INFO64: libc::c_int = 4;
const HOST_VM_INFO64_COUNT: libc::c_uint =
    (std::mem::size_of::<VmStatistics64>() / std::mem::size_of::<libc::c_int>()) as libc::c_uint;

extern "C" {
    fn mach_host_self() -> libc::c_uint;
    fn host_statistics64(
        host: libc::c_uint,
        flavor: libc::c_int,
        host_info: *mut libc::c_int,
        host_info_count: *mut libc::c_uint,
    ) -> libc::c_int;
}

fn vm_statistics() -> Result<VmStatistics64> {
    let mut stats = VmStatistics64::default();
    let mut count = HOST_VM_INFO64_COUNT;
    // SAFETY: `stats` is a correctly sized `vm_statistics64_data_t` and `count`
    // is its length in 32-bit words, which is the contract of the Mach call.
    let status = unsafe {
        host_statistics64(
            mach_host_self(),
            HOST_VM_INFO64,
            &mut stats as *mut VmStatistics64 as *mut libc::c_int,
            &mut count,
        )
    };
    if status != 0 {
        return Err(CoreError::invalid(format!(
            "host_statistics64 failed with status {status}"
        )));
    }
    Ok(stats)
}

fn sysctl_u64(name: &[u8]) -> Option<u64> {
    let mut value: u64 = 0;
    let mut size = std::mem::size_of::<u64>();
    // SAFETY: `name` is NUL-terminated, and the output buffer matches `size`.
    let status = unsafe {
        libc::sysctlbyname(
            name.as_ptr() as *const libc::c_char,
            &mut value as *mut u64 as *mut libc::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    (status == 0).then_some(value)
}

/// `struct xsw_usage` from `<sys/sysctl.h>`.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct XswUsage {
    xsu_total: u64,
    xsu_avail: u64,
    xsu_used: u64,
    xsu_pagesize: u32,
    xsu_encrypted: u8,
}

fn swap_usage() -> (Option<i64>, Option<i64>) {
    let mut usage = XswUsage::default();
    let mut size = std::mem::size_of::<XswUsage>();
    // SAFETY: output buffer matches the size the kernel expects for
    // `vm.swapusage`.
    let status = unsafe {
        libc::sysctlbyname(
            b"vm.swapusage\0".as_ptr() as *const libc::c_char,
            &mut usage as *mut XswUsage as *mut libc::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 {
        return (None, None);
    }
    (Some(usage.xsu_total as i64), Some(usage.xsu_used as i64))
}

/// macOS publishes its own pressure level. DiskMate reports that value rather
/// than deriving one, so the app and Activity Monitor never disagree.
fn pressure_level() -> MemoryPressure {
    match sysctl_u64(b"kern.memorystatus_vm_pressure_level\0") {
        Some(1) => MemoryPressure::Normal,
        Some(2) => MemoryPressure::Warning,
        Some(4) => MemoryPressure::Critical,
        _ => MemoryPressure::Unknown,
    }
}
