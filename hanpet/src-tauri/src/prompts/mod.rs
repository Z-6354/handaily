//! 提示词模板：内置 `bundled/prompts`，运行时优先读用户数据目录副本

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULTS: &[(&str, &str)] = crate::embedded::EMBEDDED_PROMPTS;

/// 首次启动：将内置模板复制到 `%AppData%/xiaohan-daily/prompts/`
pub fn seed_user_prompts(data_dir: &Path) -> std::io::Result<()> {
    let dir = prompts_dir(data_dir);
    fs::create_dir_all(&dir)?;
    for (name, content) in DEFAULTS {
        let path = dir.join(format!("{name}.md"));
        if !path.exists() {
            fs::write(&path, content)?;
        }
    }
    Ok(())
}

pub fn prompts_dir(data_dir: &Path) -> PathBuf {
    crate::data_layout::prompts_dir(data_dir)
}

/// 加载并渲染提示词（用户目录优先，否则内置默认）
pub fn render(data_dir: &Path, name: &str, vars: &[(&str, &str)]) -> String {
    let template = load_template(data_dir, name);
    substitute(&template, vars)
}

fn load_template(data_dir: &Path, name: &str) -> String {
    let user_path = prompts_dir(data_dir).join(format!("{name}.md"));
    if let Ok(s) = fs::read_to_string(&user_path) {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    DEFAULTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| c.trim().to_string())
        .unwrap_or_default()
}

fn substitute(template: &str, vars: &[(&str, &str)]) -> String {
    let map: HashMap<&str, &str> = vars.iter().copied().collect();
    let mut out = String::with_capacity(template.len());
    let mut i = 0;
    let bytes = template.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'{' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            if let Some(end) = template[i + 2..].find("}}") {
                let key = template[i + 2..i + 2 + end].trim();
                if let Some(val) = map.get(key) {
                    out.push_str(val);
                } else {
                    out.push_str(&template[i..i + 2 + end + 2]);
                }
                i += 2 + end + 2;
                continue;
            }
        }
        if let Some(ch) = template[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn substitute_replaces_placeholders() {
        let t = "Hello {{name}}, count={{n}}";
        let r = substitute(t, &[("name", "小寒"), ("n", "3")]);
        assert_eq!(r, "Hello 小寒, count=3");
    }

    #[test]
    fn seed_and_load_user_override() {
        let base = env::temp_dir().join(format!("xiaohan-prompt-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        seed_user_prompts(&base).unwrap();
        let path = prompts_dir(&base).join("vision-screenshot.md");
        assert!(path.exists());
        fs::write(&path, "custom {{app_name}}").unwrap();
        let r = render(&base, "vision-screenshot", &[("app_name", "VSCode")]);
        assert_eq!(r, "custom VSCode");
        let _ = fs::remove_dir_all(&base);
    }
}
