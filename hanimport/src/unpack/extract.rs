use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const UNPACK_SCRIPT: &str = "scripts/unpack_bundle.py";

#[derive(Debug, serde::Deserialize)]
struct UnpackJson {
    ok: bool,
    slug: Option<String>,
    kind: Option<String>,
    output_dir: Option<String>,
    files: Option<Vec<String>>,
    error: Option<String>,
}

pub struct UnpackResult {
    pub slug: String,
    pub kind: String,
    pub output_dir: PathBuf,
    pub files: Vec<String>,
}

pub fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(UNPACK_SCRIPT)
}

pub fn python_executable() -> String {
    std::env::var("HANDAILY_PYTHON")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "python".to_string())
}

pub fn check_python_deps() -> Result<(), String> {
    let python = python_executable();
    let check = Command::new(&python)
        .args(["-c", "import UnityPy"])
        .output()
        .map_err(|e| format!("failed to run {python}: {e}"))?;
    if check.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&check.stderr);
    Err(format!(
        "UnityPy not installed for {python}. Run: {python} -m pip install -r hanimport/scripts/requirements.txt\n{stderr}"
    ))
}

pub fn extract_bundle(input: &Path, output: &Path, slug: Option<&str>) -> Result<UnpackResult, String> {
    let script = script_path();
    if !script.is_file() {
        return Err(format!("unpack script not found: {}", script.display()));
    }

    let python = python_executable();
    let mut cmd = Command::new(&python);
    cmd.arg(&script)
        .arg("--input")
        .arg(input)
        .arg("--output")
        .arg(output);
    if let Some(s) = slug {
        cmd.arg("--slug").arg(s);
    }
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);

    let output_proc = cmd
        .output()
        .map_err(|e| format!("failed to spawn {python} for unpack: {e}"))?;

    let stdout = String::from_utf8_lossy(&output_proc.stdout);
    let stderr = String::from_utf8_lossy(&output_proc.stderr);

    let json_line = stdout
        .lines()
        .find(|l| l.trim_start().starts_with('{'))
        .or_else(|| stderr.lines().find(|l| l.trim_start().starts_with('{')))
        .unwrap_or("")
        .trim();

    if json_line.is_empty() {
        return Err(format!(
            "unpack produced no JSON for {} (exit {})\nstdout: {stdout}\nstderr: {stderr}",
            input.display(),
            output_proc.status.code().unwrap_or(-1)
        ));
    }

    let parsed: UnpackJson =
        serde_json::from_str(json_line).map_err(|e| format!("invalid unpack JSON: {e}\n{json_line}"))?;

    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| format!("unpack failed for {}", input.display())));
    }

    Ok(UnpackResult {
        slug: parsed.slug.ok_or("missing slug in unpack response")?,
        kind: parsed.kind.unwrap_or_else(|| "unknown".to_string()),
        output_dir: parsed
            .output_dir
            .map(PathBuf::from)
            .ok_or("missing output_dir in unpack response")?,
        files: parsed.files.unwrap_or_default(),
    })
}
