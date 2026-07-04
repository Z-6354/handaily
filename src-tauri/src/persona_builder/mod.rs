//! 人设工坊：文本预处理 → 结构化 JSON → Skill Markdown → 写入 personas

use std::path::Path;

use chrono::Local;
use rusqlite::Connection;
use serde::Serialize;

use crate::db::character_profiles::{
    self, CharacterProfileData, CharacterProfileRow,
};
use crate::persona::{self, PersonaMeta};
use crate::prompts;

#[derive(Debug, Clone, Serialize)]
pub struct CharacterOpResult {
    pub profile: CharacterProfileRow,
    pub message: String,
}

pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        format!("char-{}", Local::now().timestamp())
    } else {
        slug
    }
}

pub fn unique_slug(db: &Connection, base: &str) -> Result<String, String> {
    let slug = slugify(base);
    if !character_profiles::slug_exists(db, &slug).map_err(|e| e.to_string())? {
        return Ok(slug);
    }
    for i in 2..100 {
        let candidate = format!("{slug}-{i}");
        if !character_profiles::slug_exists(db, &candidate).map_err(|e| e.to_string())? {
            return Ok(candidate);
        }
    }
    Err("无法生成唯一 slug".into())
}

pub fn create_profile(
    db: &Connection,
    name: &str,
    source: &str,
    raw_text: &str,
) -> Result<CharacterProfileRow, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("请填写角色名".into());
    }
    let slug = unique_slug(db, name)?;
    let now = Local::now().to_rfc3339();
    let id = character_profiles::insert_profile(db, &slug, name, source.trim(), raw_text.trim(), &now)
        .map_err(|e| e.to_string())?;
    character_profiles::get_profile(db, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "创建失败".into())
}

pub fn get_profile(db: &Connection, id: i64) -> Result<CharacterProfileRow, String> {
    character_profiles::get_profile(db, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("角色资料不存在: {id}"))
}

pub fn list_profiles(db: &Connection) -> Result<Vec<CharacterProfileRow>, String> {
    character_profiles::list_profiles(db).map_err(|e| e.to_string())
}

pub fn update_profile_data(
    db: &Connection,
    id: i64,
    data: CharacterProfileData,
) -> Result<CharacterProfileRow, String> {
    let now = Local::now().to_rfc3339();
    character_profiles::update_profile_json(db, id, &data, &now).map_err(|e| e.to_string())?;
    get_profile(db, id)
}

pub fn update_raw_text(
    db: &Connection,
    id: i64,
    raw_text: &str,
) -> Result<CharacterProfileRow, String> {
    let now = Local::now().to_rfc3339();
    character_profiles::update_raw_text(db, id, raw_text.trim(), &now).map_err(|e| e.to_string())?;
    get_profile(db, id)
}

pub fn save_skill_md(
    db: &Connection,
    id: i64,
    skill_md: &str,
) -> Result<CharacterProfileRow, String> {
    let now = Local::now().to_rfc3339();
    character_profiles::update_skill_md(db, id, skill_md.trim(), &now).map_err(|e| e.to_string())?;
    get_profile(db, id)
}

pub fn delete_profile(db: &Connection, id: i64) -> Result<(), String> {
    character_profiles::delete_profile(db, id).map_err(|e| e.to_string())
}

pub fn build_preprocess_prompt(data_dir: &Path, row: &CharacterProfileRow) -> Result<String, String> {
    build_preprocess_prompt_from_text(data_dir, &row.name, &row.source, &row.raw_text)
}

const MAX_REFERENCE_CHARS: usize = 24_000;

fn clamp_reference_text(raw_text: &str) -> String {
    let t = raw_text.trim();
    if t.chars().count() <= MAX_REFERENCE_CHARS {
        return t.to_string();
    }
    let truncated: String = t.chars().take(MAX_REFERENCE_CHARS).collect();
    format!("{truncated}\n\n[参考文本过长，已截断至前 {MAX_REFERENCE_CHARS} 字]")
}

pub fn build_preprocess_prompt_from_text(
    data_dir: &Path,
    name: &str,
    source: &str,
    raw_text: &str,
) -> Result<String, String> {
    if raw_text.trim().is_empty() {
        return Err("参考文本不能为空".into());
    }
    Ok(prompts::render(
        data_dir,
        "persona-preprocess",
        &[
            ("name", name),
            ("source", source),
            ("raw_text", &clamp_reference_text(raw_text)),
        ],
    ))
}

pub fn build_merge_prompt_from_profile(
    data_dir: &Path,
    existing: &CharacterProfileData,
    new_text: &str,
) -> Result<String, String> {
    if new_text.trim().is_empty() {
        return Err("补充文本不能为空".into());
    }
    let existing_json = serde_json::to_string_pretty(existing).map_err(|e| e.to_string())?;
    Ok(prompts::render(
        data_dir,
        "persona-text-merge",
        &[
            ("existing_json", &existing_json),
            ("new_text", new_text.trim()),
        ],
    ))
}

pub fn profile_has_content(profile: &CharacterProfileData) -> bool {
    !profile.name.trim().is_empty()
        || !profile.introduction.trim().is_empty()
        || !profile.personality.is_empty()
        || !profile.speech_style.trim().is_empty()
        || !profile.sample_lines.is_empty()
        || !profile.relationships.trim().is_empty()
        || !profile.taboos.is_empty()
        || !profile.extra.is_empty()
}

pub fn build_skill_prompt_from_profile(
    data_dir: &Path,
    profile: &CharacterProfileData,
) -> Result<String, String> {
    if !profile_has_content(profile) {
        return Err("结构化资料为空，无法生成 Skill".into());
    }
    let profile_json = serde_json::to_string_pretty(profile).map_err(|e| e.to_string())?;
    Ok(prompts::render(
        data_dir,
        "persona-skill-generate",
        &[("profile_json", &profile_json)],
    ))
}

pub fn save_processed_persona(
    data_dir: &Path,
    id: &str,
    profile: &CharacterProfileData,
    skill_md: &str,
    name_hint: Option<&str>,
    source_hint: Option<&str>,
) -> Result<PersonaMeta, String> {
    let skill = skill_md.trim();
    if skill.is_empty() {
        return Err("Skill 文档为空".into());
    }
    persona::save_persona_file(data_dir, id, skill)?;
    persona::save_persona_profile(data_dir, id, profile)?;

    let manifest = persona::load_manifest(data_dir);
    let existing = manifest.personas.iter().find(|p| p.id == id);

    let name = {
        let n = profile.name.trim();
        if !n.is_empty() {
            n.to_string()
        } else if let Some(h) = name_hint.filter(|s| !s.trim().is_empty()) {
            h.trim().to_string()
        } else {
            existing
                .map(|p| p.name.clone())
                .unwrap_or_else(|| id.to_string())
        }
    };

    let source = {
        let s = profile.source.trim();
        if !s.is_empty() {
            s.to_string()
        } else if let Some(h) = source_hint.filter(|s| !s.trim().is_empty()) {
            h.trim().to_string()
        } else {
            existing.map(|p| p.source.clone()).unwrap_or_default()
        }
    };

    let desc = if profile.introduction.chars().count() > 80 {
        format!(
            "{}…",
            profile.introduction.chars().take(80).collect::<String>()
        )
    } else {
        profile.introduction.clone()
    };

    let meta = PersonaMeta {
        id: id.to_string(),
        name,
        source,
        description: if desc.is_empty() {
            existing
                .and_then(|p| {
                    let d = p.description.trim();
                    if d.is_empty() {
                        None
                    } else {
                        Some(d.to_string())
                    }
                })
                .unwrap_or_else(|| format!("AI 生成人设 · {id}"))
        } else {
            desc
        },
    };

    persona::upsert_manifest_entry(data_dir, &meta)?;
    Ok(meta)
}

pub fn apply_preprocessed(
    db: &Connection,
    id: i64,
    raw: &str,
) -> Result<CharacterOpResult, String> {
    let data = parse_profile_json(raw)?;
    let profile = update_profile_data(db, id, data)?;
    Ok(CharacterOpResult {
        profile,
        message: "文本已解析为结构化资料".into(),
    })
}

pub fn build_merge_prompt(
    data_dir: &Path,
    row: &CharacterProfileRow,
    new_text: &str,
) -> Result<String, String> {
    let existing = serde_json::to_string_pretty(&row.profile_json).map_err(|e| e.to_string())?;
    Ok(prompts::render(
        data_dir,
        "persona-text-merge",
        &[
            ("existing_json", &existing),
            ("new_text", new_text.trim()),
        ],
    ))
}

pub fn apply_merged(
    db: &Connection,
    id: i64,
    raw: &str,
) -> Result<CharacterOpResult, String> {
    apply_preprocessed(db, id, raw).map(|mut r| {
        r.message = "已合并补充文本到角色资料".into();
        r
    })
}

pub fn build_skill_prompt(data_dir: &Path, row: &CharacterProfileRow) -> Result<String, String> {
    if row.profile_json.name.trim().is_empty() && row.profile_json.introduction.trim().is_empty() {
        return Err("请先完成文本解析或填写结构化资料".into());
    }
    let profile_json = serde_json::to_string_pretty(&row.profile_json).map_err(|e| e.to_string())?;
    Ok(prompts::render(
        data_dir,
        "persona-skill-generate",
        &[("profile_json", &profile_json)],
    ))
}

pub fn apply_generated_skill(
    db: &Connection,
    id: i64,
    raw: &str,
) -> Result<CharacterOpResult, String> {
    let skill_md = strip_md_fence(raw);
    let profile = save_skill_md(db, id, &skill_md)?;
    Ok(CharacterOpResult {
        profile,
        message: "Skill 文档已生成".into(),
    })
}

pub fn apply_to_persona(
    db: &Connection,
    data_dir: &Path,
    id: i64,
    activate: bool,
) -> Result<CharacterOpResult, String> {
    let row = get_profile(db, id)?;
    let skill = row.skill_md.trim();
    if skill.is_empty() {
        return Err("请先生成或编写 Skill 文档".into());
    }
    let persona_id = row.persona_id.clone().unwrap_or_else(|| row.slug.clone());
    let name = {
        let n = row.profile_json.name.trim();
        if n.is_empty() {
            row.name.clone()
        } else {
            n.to_string()
        }
    };
    let source = {
        let s = row.profile_json.source.trim();
        if s.is_empty() {
            row.source.clone()
        } else {
            s.to_string()
        }
    };
    let desc = if row.profile_json.introduction.chars().count() > 80 {
        format!(
            "{}…",
            row.profile_json
                .introduction
                .chars()
                .take(80)
                .collect::<String>()
        )
    } else {
        row.profile_json.introduction.clone()
    };
    let meta = PersonaMeta {
        id: persona_id.clone(),
        name,
        source,
        description: if desc.is_empty() {
            format!("由人设工坊生成 · {}", row.name)
        } else {
            desc
        },
    };
    persona::save_persona_file(data_dir, &persona_id, skill)?;
    if !row.profile_json.name.is_empty() || !row.profile_json.introduction.is_empty() {
        let json = serde_json::to_string_pretty(&row.profile_json).map_err(|e| e.to_string())?;
        let json_path = persona::personas_dir(data_dir).join(format!("{persona_id}.json"));
        std::fs::write(json_path, json).map_err(|e| e.to_string())?;
    }
    persona::upsert_manifest_entry(data_dir, &meta)?;
    let now = Local::now().to_rfc3339();
    character_profiles::set_persona_id(db, id, &persona_id, &now).map_err(|e| e.to_string())?;
    if activate {
        let manifest = persona::load_manifest(data_dir);
        persona::set_active_persona_id(db, &manifest, &persona_id)?;
    }
    let profile = get_profile(db, id)?;
    Ok(CharacterOpResult {
        profile,
        message: if activate {
            format!("已写入人设「{}」并设为当前", meta.name)
        } else {
            format!("已写入人设「{}」", meta.name)
        },
    })
}

pub fn parse_profile_json(raw: &str) -> Result<CharacterProfileData, String> {
    parse_profile_json_with_repair(raw, false)
}

pub fn parse_profile_json_with_repair(raw: &str, already_retried: bool) -> Result<CharacterProfileData, String> {
    let trimmed = extract_json_str(raw.trim());
    if trimmed.is_empty() {
        let preview: String = raw.trim().chars().take(200).collect();
        return Err(if preview.is_empty() {
            "AI 返回内容为空，无法解析 JSON。请检查思考模型配置或更换模型。".into()
        } else {
            format!(
                "AI 返回内容中未找到 JSON。回复开头：{preview}{}",
                if raw.chars().count() > 200 { "…" } else { "" }
            )
        });
    }

    match try_parse_profile_value(&trimmed) {
        Ok(data) => Ok(data),
        Err(first_err) if is_truncated_json_error(&first_err) => {
            let repaired = close_json_fragment(&trimmed);
            match try_parse_profile_value(&repaired) {
                Ok(data) => Ok(data),
                Err(_) if !already_retried => Err(truncated_json_err(&first_err)),
                Err(repair_err) => Err(truncated_json_err(&repair_err)),
            }
        }
        Err(e) => Err(format!("无法解析 AI 返回的 JSON: {e}")),
    }
}

fn try_parse_profile_value(json_str: &str) -> Result<CharacterProfileData, String> {
    let v: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| e.to_string())?;
    serde_json::from_value(v).map_err(|e| format!("JSON 字段不符合角色资料格式: {e}"))
}

fn is_truncated_json_error(err: &str) -> bool {
    err.contains("EOF")
        || err.contains("expected")
        || err.contains("trailing")
        || err.contains("invalid type")
}

fn truncated_json_err(detail: &str) -> String {
    format!(
        "AI 返回的 JSON 不完整（可能被截断）: {detail}。请缩短参考文本、减少 sample_lines，或更换输出上限更高的模型。"
    )
}

/// 尝试闭合被 max_tokens 截断的 JSON 片段
fn close_json_fragment(s: &str) -> String {
    let mut result = String::new();
    let mut in_string = false;
    let mut escape = false;
    let mut stack: Vec<char> = Vec::new();

    for ch in s.chars() {
        result.push(ch);
        if escape {
            escape = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape = true,
            '"' => in_string = !in_string,
            '{' if !in_string => stack.push('}'),
            '[' if !in_string => stack.push(']'),
            '}' | ']' if !in_string => {
                if stack.last() == Some(&ch) {
                    stack.pop();
                }
            }
            _ => {}
        }
    }

    if in_string {
        result.push('"');
    }

    trim_trailing_json_noise(&mut result);

    if result.trim_end().ends_with(':') {
        result.push_str(" null");
    }

    while let Some(closer) = stack.pop() {
        result.push(closer);
    }

    result
}

fn trim_trailing_json_noise(s: &mut String) {
    while let Some(last) = s.chars().last() {
        if last == ',' || last == ':' || last.is_whitespace() {
            s.pop();
        } else if last == '"' {
            // 截断在未闭合的键名中间，去掉不完整键
            if let Some(colon) = s.rfind(':') {
                let tail = s[colon..].trim();
                if tail == ":" || tail.starts_with(": ") && !tail.contains('{') && !tail.contains('[') {
                    s.truncate(colon);
                    trim_trailing_json_noise(s);
                    return;
                }
            }
            break;
        } else {
            break;
        }
    }
}

fn extract_json_str(s: &str) -> String {
    let t = strip_md_fence(s);
    if t.starts_with('{') {
        return t;
    }
    if let Some(start) = t.find('{') {
        if let Some(end) = t.rfind('}') {
            return t[start..=end].to_string();
        }
    }
    t
}

pub fn strip_md_fence(s: &str) -> String {
    let t = s.trim();
    if t.starts_with("```") {
        let inner = t
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        return inner.to_string();
    }
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_with_preamble() {
        let raw = r#"好的，这是 JSON：
{"name":"测试","source":"","introduction":"介绍","personality":[],"speech_style":"","sample_lines":[],"relationships":"","taboos":[],"extra":{}}"#;
        let data = parse_profile_json(raw).unwrap();
        assert_eq!(data.name, "测试");
    }

    #[test]
    fn repair_truncated_json() {
        let raw = r#"{"name":"测试","source":"","introduction":"介绍","personality":["a","b"],"sample_lines":["line1","line2","incomplete"#;
        let data = parse_profile_json(raw).unwrap();
        assert_eq!(data.name, "测试");
        assert!(data.personality.len() >= 2);
    }
}
