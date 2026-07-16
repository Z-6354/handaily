use std::path::Path;
use std::process::Command;

use crate::paths;

/// Run a hanpet CLI bin via workspace cargo (`xiaohan-daily`, feature `cli-tools`).
pub fn run_hanpet_bin(bin: &str, args: &[String]) -> Result<(), String> {
    let root = paths::project_root();
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&root).args([
        "run",
        "-p",
        "xiaohan-daily",
        "--features",
        "cli-tools",
        "--bin",
        bin,
        "--",
    ]);
    for arg in args {
        cmd.arg(arg);
    }

    eprintln!("[hanimport] cargo run -p xiaohan-daily --bin {bin} -- ...");
    let status = cmd.status().map_err(|e| format!("failed to spawn cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{bin} exited with code {}",
            status.code().unwrap_or(-1)
        ))
    }
}

/// Generate live2d import plan via `mcp/blhx-wiki` npm script.
pub fn run_live2d_plan(
    out: &Path,
    live2d_root: Option<&Path>,
    min_score: u32,
    only_with_persona: bool,
) -> Result<(), String> {
    paths::ensure_parent_dir(out).map_err(|e| e.to_string())?;

    let wiki_dir = paths::blhx_wiki_dir();
    if !wiki_dir.join("package.json").is_file() {
        return Err(format!(
            "blhx-wiki not found at {}",
            wiki_dir.display()
        ));
    }

    let out_abs = out
        .canonicalize()
        .unwrap_or_else(|_| out.to_path_buf());

    let mut args = vec![
        "run".to_string(),
        "live2d-plan".to_string(),
        "--".to_string(),
        "--out".to_string(),
        out_abs.display().to_string(),
        "--min-score".to_string(),
        min_score.to_string(),
    ];
    if let Some(root) = live2d_root {
        args.push("--live2d-root".to_string());
        args.push(root.display().to_string());
    }
    if only_with_persona {
        args.push("--only-with-persona".to_string());
    } else {
        args.push("--all-personas".to_string());
    }

    let mut cmd = Command::new("npm");
    cmd.current_dir(&wiki_dir).args(&args);

    eprintln!(
        "[hanimport] npm run live2d-plan (cwd: {})",
        wiki_dir.display()
    );
    let status = cmd.status().map_err(|e| format!("failed to spawn npm: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "live2d-plan exited with code {}",
            status.code().unwrap_or(-1)
        ))
    }
}
