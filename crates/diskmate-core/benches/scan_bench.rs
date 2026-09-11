//! Reproducible scanner benchmark.
//!
//! Run with `cargo bench -p diskmate-core` (or `scripts/bench.sh`). It builds a
//! synthetic tree of a known shape, scans it, and prints one row per
//! configuration so two revisions of the scanner can be compared on identical
//! input.
//!
//! It is a plain binary rather than a Criterion harness on purpose: a scan
//! takes seconds and mutates a database, so the statistical repetition
//! Criterion is built for would measure the page cache rather than the scanner.
//! What matters here is throughput on a realistic tree, reported honestly with
//! the machine and settings that produced it.
//!
//! ```text
//! cargo bench -p diskmate-core -- --files 200000 --dirs 4000
//! cargo bench -p diskmate-core -- --path /Users/you/Library --label library
//! ```
//!
//! No number printed by this harness should be quoted as a comparison against
//! another product unless that product was measured on the same tree, on the
//! same machine, in the same session.

use std::path::{Path, PathBuf};
use std::time::Instant;

use diskmate_core::model::{ScanOptions, ScanSummary};
use diskmate_core::paths::AppPaths;
use diskmate_core::services::StorageService;

struct Config {
    files: usize,
    dirs: usize,
    depth: usize,
    threads: Vec<usize>,
    batch_sizes: Vec<usize>,
    path: Option<PathBuf>,
    label: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            files: 120_000,
            dirs: 3_000,
            depth: 5,
            threads: vec![1, 2, 4, 8],
            batch_sizes: vec![2_048],
            path: None,
            label: "synthetic".into(),
        }
    }
}

fn parse_args() -> Config {
    let mut config = Config::default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        let take = |index: &mut usize| -> Option<String> {
            *index += 1;
            args.get(*index).cloned()
        };
        match args[index].as_str() {
            "--files" => {
                config.files = take(&mut index)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(config.files)
            }
            "--dirs" => {
                config.dirs = take(&mut index)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(config.dirs)
            }
            "--depth" => {
                config.depth = take(&mut index)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(config.depth)
            }
            "--threads" => {
                if let Some(value) = take(&mut index) {
                    config.threads = value.split(',').filter_map(|v| v.parse().ok()).collect();
                }
            }
            "--batch" => {
                if let Some(value) = take(&mut index) {
                    config.batch_sizes = value.split(',').filter_map(|v| v.parse().ok()).collect();
                }
            }
            "--path" => config.path = take(&mut index).map(PathBuf::from),
            "--label" => config.label = take(&mut index).unwrap_or(config.label),
            // Cargo passes `--bench` through; ignore anything unrecognised so
            // the harness stays usable from `cargo bench`.
            _ => {}
        }
        index += 1;
    }
    config
}

fn main() {
    let config = parse_args();
    println!(
        "DiskMate scanner benchmark\n  engine   {}\n  host     {} {}\n  cores    {}",
        diskmate_core::ENGINE_VERSION,
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(0),
    );

    let (tree, _keep_alive) = match &config.path {
        Some(path) => {
            println!("  target   {} (existing tree)", path.display());
            (path.clone(), None)
        }
        None => {
            let temp = tempfile::tempdir().expect("temp dir");
            let root = temp.path().to_path_buf();
            println!(
                "  target   synthetic tree: {} files, {} dirs, depth {}",
                config.files, config.dirs, config.depth
            );
            let built = Instant::now();
            build_tree(&root, config.files, config.dirs, config.depth);
            println!("  built in {:?}", built.elapsed());
            (root, Some(temp))
        }
    };

    println!(
        "\n{:<8} {:>7} {:>7} {:>12} {:>12} {:>12} {:>10} {:>10} {:>8}",
        "label",
        "threads",
        "batch",
        "entries",
        "duration",
        "entries/s",
        "indexed",
        "index db",
        "errors"
    );

    for &threads in &config.threads {
        for &batch in &config.batch_sizes {
            let run = measure(&tree, threads, batch);
            println!(
                "{:<8} {:>7} {:>7} {:>12} {:>12} {:>12} {:>10} {:>10} {:>8}",
                config.label,
                threads,
                batch,
                run.entry_count(),
                format!("{} ms", run.duration_ms),
                format!("{:.0}", run.entries_per_second),
                human_bytes(run.logical_bytes),
                human_bytes(run.index_bytes),
                run.error_count,
            );
        }
    }

    println!(
        "\nMeasured on this machine only. Comparisons against other tools are \
         valid only when both were run on the same tree in the same session."
    );
}

fn measure(tree: &Path, threads: usize, batch_size: usize) -> ScanSummary {
    // Each run gets its own index so one run never reads another's page cache
    // or pays for another's rows.
    let data = tempfile::tempdir().expect("temp data");
    let paths = AppPaths::resolve(None, Some(data.path().join("state"))).expect("resolve paths");
    let service = StorageService::new(paths).expect("open index");

    let mut options = ScanOptions::new(tree);
    options.threads = Some(threads);
    options.batch_size = batch_size;
    // Progress events are not what is being measured; keep them rare.
    options.progress_interval_ms = 1_000;

    service.scan(options, |_| {}).expect("scan")
}

/// Build a tree with a realistic shape: most files in leaf directories, a
/// spread of sizes, and a mix of extensions.
fn build_tree(root: &Path, files: usize, dirs: usize, depth: usize) {
    let mut directories = Vec::with_capacity(dirs);
    directories.push(root.to_path_buf());

    let mut index = 0usize;
    while directories.len() < dirs {
        let parent = directories[index % directories.len()].clone();
        let level = parent.components().count() - root.components().count();
        let target = if level >= depth {
            root.to_path_buf()
        } else {
            parent
        };
        let child = target.join(format!("dir-{:05}", directories.len()));
        std::fs::create_dir_all(&child).expect("create dir");
        directories.push(child);
        index += 1;
    }

    const EXTENSIONS: [&str; 6] = ["jpg", "mov", "pdf", "log", "bin", "txt"];
    let mut buffer = vec![0u8; 64 * 1024];
    for n in 0..files {
        let dir = &directories[n % directories.len()];
        let ext = EXTENSIONS[n % EXTENSIONS.len()];
        // A long tail of small files with occasional large ones, which is what
        // a real volume looks like and what stresses per-entry overhead.
        let size = match n % 100 {
            0 => 4 * 1024 * 1024,
            1..=9 => 256 * 1024,
            _ => 1 + (n % 8_192),
        };
        let path = dir.join(format!("file-{n:07}.{ext}"));
        write_sized(&path, size, &mut buffer);
    }
}

fn write_sized(path: &Path, size: usize, buffer: &mut [u8]) {
    use std::io::Write;
    let file = std::fs::File::create(path).expect("create file");
    let mut writer = std::io::BufWriter::new(file);
    let mut remaining = size;
    while remaining > 0 {
        let chunk = remaining.min(buffer.len());
        writer.write_all(&buffer[..chunk]).expect("write");
        remaining -= chunk;
    }
    writer.flush().expect("flush");
}

fn human_bytes(bytes: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
