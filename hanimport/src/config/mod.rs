mod extract;

use std::path::{Path, PathBuf};

pub use extract::ConfigResult;

pub fn discover_spine_folders(input: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if is_spine_folder(input) {
        out.push(input.to_path_buf());
        return Ok(out);
    }
    if !input.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(input)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && is_spine_folder(&path) {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

pub fn discover_cubism_folders(input: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if is_cubism_folder(input) {
        out.push(input.to_path_buf());
        return Ok(out);
    }
    if !input.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(input)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && is_cubism_folder(&path) {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

fn is_spine_folder(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    dir.read_dir()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .any(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("skel"))
        })
}

fn is_cubism_folder(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    dir.read_dir()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .any(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("moc3"))
        })
}

pub fn run_config(input: &Path, dry_run: bool, force: bool) -> Result<(), String> {
    if !input.exists() {
        return Err(format!("input not found: {}", input.display()));
    }

    let spine = discover_spine_folders(input).map_err(|e| e.to_string())?;
    let cubism = discover_cubism_folders(input).map_err(|e| e.to_string())?;
    if spine.is_empty() && cubism.is_empty() {
        return Err(format!(
            "no Spine (.skel) or Cubism (.moc3) folders found under {}",
            input.display()
        ));
    }

    eprintln!(
        "[hanimport config] spine={} cubism={}{}",
        spine.len(),
        cubism.len(),
        if dry_run { " (dry-run)" } else { "" }
    );

    for folder in &spine {
        eprintln!("  [spine] {}", folder.display());
        let result = extract::build_configs(folder, dry_run, force)?;
        eprintln!(
            "    ok: slug={} idle={:?} click={:?} touch_areas={} files=[{}]",
            result.slug,
            result.idle,
            result.click,
            result.touch_areas,
            result.written.join(", ")
        );
    }

    for folder in &cubism {
        eprintln!("  [cubism] {}", folder.display());
        let result = extract::build_cubism_configs(folder, dry_run, force)?;
        eprintln!(
            "    ok: slug={} idle={:?} click={:?} touch_areas={} files=[{}]",
            result.slug,
            result.idle,
            result.click,
            result.touch_areas,
            result.written.join(", ")
        );
    }

    if dry_run {
        eprintln!("[hanimport config] dry-run: no files written.");
    } else {
        eprintln!("[hanimport config] done.");
    }
    Ok(())
}

pub fn run_config_for_folder(folder: &Path, dry_run: bool, force: bool) -> Result<ConfigResult, String> {
    if is_cubism_folder(folder) && !is_spine_folder(folder) {
        return extract::build_cubism_configs(folder, dry_run, force);
    }
    extract::build_configs(folder, dry_run, force)
}
