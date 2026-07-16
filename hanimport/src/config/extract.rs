use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const BUILD_SCRIPT: &str = "scripts/build_model_config.py";
const CUBISM_BUILD_SCRIPT: &str = "scripts/build_cubism_config.py";

#[derive(Debug, serde::Deserialize)]
struct BuildJson {
    ok: bool,
    error: Option<String>,
    results: Option<Vec<ConfigResultJson>>,
}

#[derive(Debug, serde::Deserialize)]
struct ConfigResultJson {
    slug: Option<String>,
    idle: Option<String>,
    click: Option<String>,
    touch_areas: Option<usize>,
    written: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct ConfigResult {
    pub slug: String,
    pub idle: Option<String>,
    pub click: Option<String>,
    pub touch_areas: usize,
    pub written: Vec<String>,
}

pub fn script_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(BUILD_SCRIPT)
}

pub fn python_executable() -> String {
    std::env::var("HANDAILY_PYTHON")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "python".to_string())
}

pub fn build_configs(folder: &Path, dry_run: bool, force: bool) -> Result<ConfigResult, String> {
    let script = script_path();
    if !script.is_file() {
        return Err(format!("config script not found: {}", script.display()));
    }

    let python = python_executable();
    let mut cmd = Command::new(&python);
    cmd.arg(&script).arg("--input").arg(folder);
    if dry_run {
        cmd.arg("--dry-run");
    }
    if force {
        cmd.arg("--force");
    }
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);

    let output = cmd
        .output()
        .map_err(|e| format!("failed to spawn {python} for config build: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let json_line = stdout
        .lines()
        .chain(stderr.lines())
        .find(|l| l.trim_start().starts_with('{'))
        .unwrap_or("")
        .trim();

    if json_line.is_empty() {
        return Err(format!(
            "config build produced no JSON for {} (exit {})\nstdout: {stdout}\nstderr: {stderr}",
            folder.display(),
            output.status.code().unwrap_or(-1)
        ));
    }

    let parsed: BuildJson =
        serde_json::from_str(json_line).map_err(|e| format!("invalid config JSON: {e}\n{json_line}"))?;

    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| format!("config build failed for {}", folder.display())));
    }

    let result = parsed
        .results
        .and_then(|mut v| v.pop())
        .ok_or_else(|| format!("missing config result for {}", folder.display()))?;

    Ok(ConfigResult {
        slug: result.slug.unwrap_or_else(|| {
            folder
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("model")
                .to_string()
        }),
        idle: result.idle,
        click: result.click,
        touch_areas: result.touch_areas.unwrap_or(0),
        written: result.written.unwrap_or_default(),
    })
}

#[derive(Debug, serde::Deserialize)]
struct CubismBuildJson {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    idle: Option<String>,
    #[serde(default)]
    click: Option<String>,
    #[serde(default)]
    touch_areas: Option<usize>,
    #[serde(default)]
    wrote: Option<Vec<String>>,
    #[serde(default)]
    skipped: Option<bool>,
}

pub fn build_cubism_configs(folder: &Path, dry_run: bool, force: bool) -> Result<ConfigResult, String> {
    let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(CUBISM_BUILD_SCRIPT);
    if !script.is_file() {
        return Err(format!("cubism config script not found: {}", script.display()));
    }

    let python = python_executable();
    let mut cmd = Command::new(&python);
    cmd.arg(&script).arg("--input").arg(folder);
    if dry_run {
        cmd.arg("--dry-run");
    }
    if force {
        cmd.arg("--force");
    }
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);

    let output = cmd
        .output()
        .map_err(|e| format!("failed to spawn {python} for cubism config: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let json_line = stdout
        .lines()
        .chain(stderr.lines())
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .unwrap_or("")
        .trim();

    if json_line.is_empty() {
        return Err(format!(
            "cubism config produced no JSON for {} (exit {})\nstdout: {stdout}\nstderr: {stderr}",
            folder.display(),
            output.status.code().unwrap_or(-1)
        ));
    }

    let parsed: CubismBuildJson = serde_json::from_str(json_line)
        .map_err(|e| format!("invalid cubism config JSON: {e}\n{json_line}"))?;

    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| format!("cubism config failed for {}", folder.display())));
    }

    let written = if parsed.skipped.unwrap_or(false) {
        vec!["skipped".into()]
    } else {
        parsed.wrote.unwrap_or_else(|| {
            vec![
                "animations.meta.json".into(),
                "touch_areas.json".into(),
            ]
        })
    };

    Ok(ConfigResult {
        slug: parsed.slug.unwrap_or_else(|| {
            folder
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("model")
                .to_string()
        }),
        idle: parsed.idle,
        click: parsed.click,
        touch_areas: parsed.touch_areas.unwrap_or(0),
        written,
    })
}
