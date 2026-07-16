mod complete;
mod extract;

pub use complete::{
    is_hx_slug, is_unpack_complete, prepare_unpack_dir, purge_hx_output_dirs, PrepareAction,
};

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

const BUNDLE_EXTENSIONS: &[&str] = &["ab", "unity3d", "bytes"];
const UNITYFS_MAGIC: &[u8; 7] = b"UnityFS";

#[derive(Debug, Clone)]
pub struct BundleCandidate {
    pub path: PathBuf,
    pub slug: String,
}

pub fn discover_bundles(input: &Path) -> std::io::Result<Vec<BundleCandidate>> {
    let mut out = Vec::new();
    if input.is_file() {
        if let Some(slug) = bundle_slug(input) {
            out.push(BundleCandidate {
                path: input.to_path_buf(),
                slug,
            });
        }
        return Ok(out);
    }
    if !input.is_dir() {
        return Ok(out);
    }
    walk_bundles(input, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn walk_bundles(dir: &Path, out: &mut Vec<BundleCandidate>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_bundles(&path, out)?;
            continue;
        }
        if let Some(slug) = bundle_slug(&path) {
            out.push(BundleCandidate { path, slug });
        }
    }
    Ok(())
}

fn bundle_slug(path: &Path) -> Option<String> {
    if !is_bundle_file(path) {
        return None;
    }
    let stem = path
        .file_stem()
        .or_else(|| path.file_name())
        .and_then(|s| s.to_str())?;
    Some(stem.to_ascii_lowercase())
}

fn is_bundle_file(path: &Path) -> bool {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| BUNDLE_EXTENSIONS.iter().any(|ext| e.eq_ignore_ascii_case(ext)))
        .unwrap_or(false)
    {
        return true;
    }
    is_unityfs_file(path)
}

fn is_unityfs_file(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    use std::io::Read;
    let mut magic = [0u8; 7];
    file.read_exact(&mut magic).is_ok() && magic == *UNITYFS_MAGIC
}

pub fn default_jobs() -> usize {
    std::env::var("HANIMPORT_UNPACK_JOBS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n >= 1)
        .unwrap_or_else(|| {
            thread::available_parallelism()
                .map(|n| n.get().clamp(2, 8))
                .unwrap_or(4)
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemStatus {
    Ok,
    Skipped,
    Failed,
}

fn print_progress(
    done: usize,
    total: usize,
    ok: usize,
    skip: usize,
    fail: usize,
    running: usize,
    last: &str,
) {
    let pct = if total == 0 { 100 } else { done * 100 / total };
    eprint!(
        "\r[hanimport unpack] [{done}/{total} {pct}%] ok={ok} skip={skip} fail={fail} running={running} last={last}    "
    );
    let _ = std::io::Write::flush(&mut std::io::stderr());
}

pub fn run_unpack(
    input: &Path,
    output: &Path,
    dry_run: bool,
    with_config: bool,
    jobs: usize,
) -> Result<(), String> {
    if !input.exists() {
        return Err(format!("input not found: {}", input.display()));
    }

    let all_bundles = discover_bundles(input).map_err(|e| e.to_string())?;
    if all_bundles.is_empty() {
        return Err(format!(
            "no AssetBundle files found under {} (extensions: {} or UnityFS magic)",
            input.display(),
            BUNDLE_EXTENSIONS.join(", ")
        ));
    }

    let (bundles, hx_bundles): (Vec<_>, Vec<_>) = all_bundles
        .into_iter()
        .partition(|b| !is_hx_slug(&b.slug));

    if !dry_run {
        crate::paths::ensure_dir(output).map_err(|e| e.to_string())?;
        for name in purge_hx_output_dirs(output)? {
            eprintln!("[hanimport unpack] 清理(hx) {name}");
        }
        if !bundles.is_empty() {
            extract::check_python_deps()?;
        }
    }

    for bundle in &hx_bundles {
        eprintln!(
            "[hanimport unpack] 跳过(hx) {} -> {}/",
            bundle.path.display(),
            bundle.slug
        );
    }

    let jobs = jobs.max(1);
    eprintln!(
        "[hanimport unpack] found {} bundle(s) (hx skip {}), jobs={jobs}, output: {}{}",
        bundles.len() + hx_bundles.len(),
        hx_bundles.len(),
        output.display(),
        if dry_run { " (dry-run)" } else { "" }
    );

    if dry_run {
        for bundle in &bundles {
            let out_dir = complete::output_dir_for(output, &bundle.slug);
            let status = if is_unpack_complete(&out_dir, &bundle.slug) {
                "skip(complete)"
            } else if out_dir.exists() {
                "re-unpack(incomplete→delete)"
            } else {
                "unpack"
            };
            eprintln!(
                "  - {} -> {}/  [{status}]",
                bundle.path.display(),
                bundle.slug
            );
        }
        eprintln!("[hanimport unpack] dry-run: no files written.");
        return Ok(());
    }

    if bundles.is_empty() {
        eprintln!(
            "[hanimport unpack] summary: ok=0 skip={} fail=0 (total={})",
            hx_bundles.len(),
            hx_bundles.len()
        );
        eprintln!("[hanimport unpack] done.");
        return Ok(());
    }

    let hx_skip = hx_bundles.len();
    let total = bundles.len() + hx_skip;
    let ok_n = Arc::new(AtomicUsize::new(0));
    let skip_n = Arc::new(AtomicUsize::new(hx_skip));
    let fail_n = Arc::new(AtomicUsize::new(0));
    let done_n = Arc::new(AtomicUsize::new(hx_skip));
    let running_n = Arc::new(AtomicUsize::new(0));
    let last_slug = Arc::new(Mutex::new(String::new()));
    let fail_samples = Arc::new(Mutex::new(Vec::<String>::new()));

    let output = Arc::new(output.to_path_buf());
    let queue: Arc<Mutex<Vec<BundleCandidate>>> = Arc::new(Mutex::new(bundles));

    let workers: Vec<_> = (0..jobs)
        .map(|_| {
            let queue = Arc::clone(&queue);
            let output = Arc::clone(&output);
            let ok_n = Arc::clone(&ok_n);
            let skip_n = Arc::clone(&skip_n);
            let fail_n = Arc::clone(&fail_n);
            let done_n = Arc::clone(&done_n);
            let running_n = Arc::clone(&running_n);
            let last_slug = Arc::clone(&last_slug);
            let fail_samples = Arc::clone(&fail_samples);
            thread::spawn(move || {
                loop {
                    let bundle = {
                        let mut q = queue.lock().unwrap();
                        q.pop()
                    };
                    let Some(bundle) = bundle else { break };

                    running_n.fetch_add(1, Ordering::Relaxed);
                    if let Ok(mut last) = last_slug.lock() {
                        *last = bundle.slug.clone();
                    }
                    print_progress(
                        done_n.load(Ordering::Relaxed),
                        total,
                        ok_n.load(Ordering::Relaxed),
                        skip_n.load(Ordering::Relaxed),
                        fail_n.load(Ordering::Relaxed),
                        running_n.load(Ordering::Relaxed),
                        &bundle.slug,
                    );

                    let status = process_one(&bundle, &output);

                    running_n.fetch_sub(1, Ordering::Relaxed);
                    match status {
                        ItemStatus::Ok => {
                            ok_n.fetch_add(1, Ordering::Relaxed);
                        }
                        ItemStatus::Skipped => {
                            skip_n.fetch_add(1, Ordering::Relaxed);
                        }
                        ItemStatus::Failed => {
                            fail_n.fetch_add(1, Ordering::Relaxed);
                            if let Ok(mut samples) = fail_samples.lock() {
                                if samples.len() < 8 {
                                    samples.push(bundle.slug.clone());
                                }
                            }
                        }
                    }
                    let done = done_n.fetch_add(1, Ordering::Relaxed) + 1;
                    print_progress(
                        done,
                        total,
                        ok_n.load(Ordering::Relaxed),
                        skip_n.load(Ordering::Relaxed),
                        fail_n.load(Ordering::Relaxed),
                        running_n.load(Ordering::Relaxed),
                        &bundle.slug,
                    );
                }
            })
        })
        .collect();

    for w in workers {
        let _ = w.join();
    }
    eprintln!();

    let ok = ok_n.load(Ordering::Relaxed);
    let skip = skip_n.load(Ordering::Relaxed);
    let fail = fail_n.load(Ordering::Relaxed);
    eprintln!("[hanimport unpack] summary: ok={ok} skip={skip} fail={fail} (total={total})");
    if fail > 0 {
        if let Ok(samples) = fail_samples.lock() {
            if !samples.is_empty() {
                eprintln!("[hanimport unpack] fail samples: {}", samples.join(", "));
            }
        }
    }

    if with_config {
        eprintln!("[hanimport unpack] generating JSON configs for output tree …");
        if let Err(err) = crate::config::run_config(output.as_path(), false, false) {
            eprintln!("[hanimport unpack] config warning: {err}");
        }
    }

    eprintln!("[hanimport unpack] done.");
    if fail > 0 && ok + skip == 0 {
        return Err(format!("all {fail} unpack(s) failed"));
    }
    Ok(())
}

fn process_one(bundle: &BundleCandidate, output: &Path) -> ItemStatus {
    let out_dir = complete::output_dir_for(output, &bundle.slug);
    if is_hx_slug(&bundle.slug) {
        if out_dir.exists() {
            let _ = std::fs::remove_dir_all(&out_dir);
        }
        eprintln!("\n[hanimport unpack] 跳过(hx) {}", bundle.slug);
        return ItemStatus::Skipped;
    }
    match prepare_unpack_dir(&out_dir, &bundle.slug) {
        Ok(PrepareAction::Skip) => return ItemStatus::Skipped,
        Ok(PrepareAction::Ready) => {}
        Err(err) => {
            eprintln!("\n    {} prepare failed: {err}", bundle.slug);
            return ItemStatus::Failed;
        }
    }

    match extract::extract_bundle(&bundle.path, output, Some(&bundle.slug)) {
        Ok(result) => {
            if result.kind == "spine" {
                let _ = crate::config::run_config_for_folder(&result.output_dir, false, false);
            }
            ItemStatus::Ok
        }
        Err(err) => {
            eprintln!("\n    {} failed: {err}", bundle.slug);
            // Drop half-finished leftovers so a retry is clean.
            if out_dir.exists() && !is_unpack_complete(&out_dir, &bundle.slug) {
                let _ = std::fs::remove_dir_all(&out_dir);
            }
            ItemStatus::Failed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn discovers_bundle_by_extension() {
        let dir = tempdir().unwrap();
        let ab = dir.path().join("ship.ab");
        fs::write(&ab, b"x").unwrap();
        fs::write(dir.path().join("readme.txt"), b"nope").unwrap();
        let found = discover_bundles(dir.path()).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, ab);
        assert_eq!(found[0].slug, "ship");
    }

    #[test]
    fn discovers_extensionless_unityfs() {
        let dir = tempdir().unwrap();
        let bundle = dir.path().join("aidang_2");
        fs::write(&bundle, b"UnityFS\x00\x00\x00\x00\x08").unwrap();
        let found = discover_bundles(dir.path()).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].slug, "aidang_2");
    }
}
