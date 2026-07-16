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

/// `CharacterProfileData.extra` 中记录 AI 性格结构化更新的本地时间
pub const EXTRA_AI_PROFILE_UPDATED_AT: &str = "ai_profile_updated_at";

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
const MAX_REFERENCE_CHARS_COMPACT: usize = 8_000;
const MAX_REFERENCE_CHARS_MINIMAL: usize = 4_000;
const MAX_REFERENCE_CHARS_ULTRA: usize = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreprocessMode {
    Standard,
    Compact,
    Minimal,
}

fn clamp_reference_text_to(raw_text: &str, max_chars: usize) -> String {
    let t = raw_text.trim();
    if t.chars().count() <= max_chars {
        return t.to_string();
    }
    let truncated: String = t.chars().take(max_chars).collect();
    format!("{truncated}\n\n[参考文本过长，已截断至前 {max_chars} 字]")
}

pub fn build_preprocess_prompt_from_text(
    data_dir: &Path,
    name: &str,
    source: &str,
    raw_text: &str,
) -> Result<String, String> {
    build_preprocess_prompt_from_text_mode(
        data_dir,
        name,
        source,
        raw_text,
        PreprocessMode::Standard,
    )
}

pub fn build_preprocess_prompt_from_text_mode(
    data_dir: &Path,
    name: &str,
    source: &str,
    raw_text: &str,
    mode: PreprocessMode,
) -> Result<String, String> {
    build_preprocess_prompt_limited(data_dir, name, source, raw_text, mode, None)
}

pub fn build_preprocess_prompt_limited(
    data_dir: &Path,
    name: &str,
    source: &str,
    raw_text: &str,
    mode: PreprocessMode,
    max_chars: Option<usize>,
) -> Result<String, String> {
    if raw_text.trim().is_empty() {
        return Err("参考文本不能为空".into());
    }
    let (template, default_max) = match mode {
        PreprocessMode::Standard => ("persona-preprocess", MAX_REFERENCE_CHARS),
        PreprocessMode::Compact => ("persona-preprocess-compact", MAX_REFERENCE_CHARS_COMPACT),
        PreprocessMode::Minimal => ("persona-preprocess-minimal", MAX_REFERENCE_CHARS_MINIMAL),
    };
    let limit = max_chars.unwrap_or(default_max);
    Ok(prompts::render(
        data_dir,
        template,
        &[
            ("name", name),
            ("source", source),
            ("raw_text", &clamp_reference_text_to(raw_text, limit)),
        ],
    ))
}

/// Wiki 等人设导入前的预处理档位：(模式, 参考文本上限)
pub fn preprocess_attempt_tiers(from_wiki: bool, text_len: usize) -> Vec<(PreprocessMode, usize)> {
    if from_wiki {
        return vec![
            (PreprocessMode::Minimal, MAX_REFERENCE_CHARS_MINIMAL),
            (PreprocessMode::Minimal, 1_500),
            (PreprocessMode::Compact, 2_500),
            (PreprocessMode::Minimal, MAX_REFERENCE_CHARS_ULTRA),
        ];
    }
    if text_len > MAX_REFERENCE_CHARS_COMPACT {
        vec![
            (PreprocessMode::Standard, MAX_REFERENCE_CHARS),
            (PreprocessMode::Compact, MAX_REFERENCE_CHARS_COMPACT),
            (PreprocessMode::Minimal, MAX_REFERENCE_CHARS_MINIMAL),
        ]
    } else if text_len > 4_000 {
        vec![
            (PreprocessMode::Standard, MAX_REFERENCE_CHARS),
            (PreprocessMode::Minimal, MAX_REFERENCE_CHARS_MINIMAL),
        ]
    } else {
        vec![
            (PreprocessMode::Standard, MAX_REFERENCE_CHARS),
            (PreprocessMode::Compact, MAX_REFERENCE_CHARS_COMPACT),
        ]
    }
}

/// 从 Wiki 清洗后的参考文本本地解析结构化资料，避免思考模型 JSON 被截断
pub fn try_profile_from_wiki_reference(
    text: &str,
    name_fallback: &str,
    source_fallback: &str,
) -> Option<CharacterProfileData> {
    let sections = split_wiki_reference_sections(text);
    let name = sections
        .title_name
        .clone()
        .or_else(|| parse_field_line(text, "角色"))
        .unwrap_or_else(|| name_fallback.trim().to_string());
    if name.is_empty() {
        return None;
    }

    let source = sections
        .source
        .clone()
        .unwrap_or_else(|| source_fallback.trim().to_string());

    let info = sections.section_text("角色信息").unwrap_or_default();
    let mut personality = parse_info_list_field(&info, "性格");
    if personality.is_empty() {
        personality = parse_info_list_field(&info, "关键词");
    }

    let mut introduction_parts: Vec<String> = Vec::new();
    if let Some(setting) = sections
        .section_text("角色设定")
        .map(|s| clean_setting_prose(&s))
        .filter(|s| !s.is_empty())
    {
        introduction_parts.push(setting);
    } else if let Some(id) = parse_info_scalar_field(&info, "身份") {
        introduction_parts.push(id);
    }

    let mut extra = std::collections::HashMap::new();
    for key in ["关键词", "发色", "瞳色", "萌点", "持有物", "身份"] {
        if let Some(v) = parse_info_scalar_field(&info, key) {
            extra.insert(key.to_string(), truncate_chars(&v, 80));
        }
    }

    let sample_lines: Vec<String> = sections
        .dialogue_lines
        .iter()
        .take(8)
        .map(|s| truncate_chars(s, 120))
        .collect();

    let speech_style = build_speech_style(&info, &personality, &sample_lines);

    let introduction = truncate_chars(&introduction_parts.join("\n\n"), 600);

    if personality.is_empty() && sample_lines.len() < 2 && introduction.chars().count() < 30 {
        return None;
    }

    Some(CharacterProfileData {
        name,
        source,
        introduction,
        personality,
        speech_style,
        sample_lines,
        relationships: String::new(),
        taboos: Vec::new(),
        extra,
    })
}

struct WikiReferenceSections {
    title_name: Option<String>,
    source: Option<String>,
    sections: std::collections::HashMap<String, String>,
    dialogue_lines: Vec<String>,
}

impl WikiReferenceSections {
    fn section_text(&self, key: &str) -> Option<String> {
        self.sections.get(key).cloned()
    }
}

fn split_wiki_reference_sections(text: &str) -> WikiReferenceSections {
    let mut title_name = None;
    let mut source = None;
    let mut sections: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut dialogue_lines: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    let mut buf = String::new();

    let flush = |key: &mut Option<String>, buffer: &mut String, map: &mut std::collections::HashMap<String, String>| {
        if let Some(k) = key.take() {
            let body = buffer.trim().to_string();
            if !body.is_empty() {
                map.insert(k, body);
            }
        }
        buffer.clear();
    };

    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("# 角色：") || t.starts_with("# 角色:") {
            title_name = Some(t.trim_start_matches("# 角色：").trim_start_matches("# 角色:").trim().to_string());
            continue;
        }
        if t.starts_with("来源：") || t.starts_with("来源:") {
            source = Some(t.trim_start_matches("来源：").trim_start_matches("来源:").trim().to_string());
            continue;
        }
        if t.starts_with("Wiki：") || t.starts_with("Wiki:") {
            continue;
        }
        if let Some(rest) = t.strip_prefix("## ") {
            flush(&mut current, &mut buf, &mut sections);
            let heading = rest
                .split('（')
                .next()
                .unwrap_or(rest)
                .trim()
                .to_string();
            current = Some(heading);
            continue;
        }
        if current.as_deref().is_some_and(|k| k.starts_with("舰船台词")) {
            if let Some(line) = t.strip_prefix("- ").map(str::trim).filter(|s| !s.is_empty()) {
                dialogue_lines.push(line.to_string());
            }
            continue;
        }
        if current.is_some() && !t.is_empty() {
            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(t);
        }
    }
    flush(&mut current, &mut buf, &mut sections);

    WikiReferenceSections {
        title_name,
        source,
        sections,
        dialogue_lines,
    }
}

fn parse_field_line(text: &str, field: &str) -> Option<String> {
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with(field) {
            let rest = t.trim_start_matches(field).trim_start_matches(['：', ':', ' ']);
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
    }
    None
}

fn trim_at_next_info_field(s: &str) -> String {
    const MARKERS: &[&str] = &[
        " 关键词", " 持有物", " 发色", " 瞳色", " 萌点", " CV ", " 画师", " 微博",
        " 推特", " PIXIV", " B站", " 5sing",
    ];
    let mut end = s.chars().count();
    for marker in MARKERS {
        if let Some(pos) = s.find(marker) {
            end = end.min(pos);
        }
    }
    s.chars().take(end).collect::<String>().trim().to_string()
}

fn parse_info_scalar_field(info: &str, field: &str) -> Option<String> {
    for line in info.lines() {
        let t = line.trim();
        if t.starts_with(field) {
            let rest = t.trim_start_matches(field).trim_start_matches(['：', ':', ' ']);
            if !rest.is_empty() && rest != field {
                return Some(trim_at_next_info_field(rest));
            }
        }
    }
    None
}

fn parse_info_list_field(info: &str, field: &str) -> Vec<String> {
    let Some(raw) = parse_info_scalar_field(info, field) else {
        return Vec::new();
    };
    raw.split(['、', '，', ',', ';', '；'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| truncate_chars(s, 40))
        .take(8)
        .collect()
}

fn clean_setting_prose(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.contains(".gif"))
        .filter(|line| !line.starts_with("翻译："))
        .filter(|line| !line.starts_with("Azurlane"))
        .filter(|line| !line.starts_with("|"))
        .filter(|line| line.chars().count() >= 4)
        .take(6)
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_speech_style(info: &str, personality: &[String], sample_lines: &[String]) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(kw) = parse_info_scalar_field(info, "关键词") {
        parts.push(format!("口癖/关键词：{}", truncate_chars(&kw, 80)));
    }
    for p in personality {
        let lower = p.to_lowercase();
        if lower.contains("口癖") || lower.contains("称呼") || lower.contains("说话") {
            parts.push(p.clone());
        }
    }
    if sample_lines.iter().any(|l| l.contains('~') || l.contains('～')) {
        parts.push("语气活泼，常用波浪号".into());
    }
    if sample_lines
        .iter()
        .any(|l| l.contains("亲爱的") || l.contains("旦那") || l.contains("指挥官"))
    {
        parts.push("亲昵称呼指挥官".into());
    }
    if parts.is_empty() && !personality.is_empty() {
        parts.push(truncate_chars(&personality.join("、"), 80));
    }
    truncate_chars(&parts.join("；"), 200)
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    format!("{}…", s.chars().take(max).collect::<String>())
}

pub fn is_truncated_profile_error(err: &str) -> bool {
    err.contains("不完整")
        || err.contains("EOF")
        || err.contains("无法解析 AI 返回的 JSON")
        || err.contains("JSON 字段不符合")
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

/// 简介/介绍/说话风格/性格等结构化字段是否齐全（批量导入后常缺此项）
pub fn profile_is_structured(profile: &CharacterProfileData) -> bool {
    !profile.introduction.trim().is_empty()
        && (!profile.personality.is_empty() || !profile.speech_style.trim().is_empty())
}

pub fn stamp_ai_profile_updated(profile: &mut CharacterProfileData) {
    profile.extra.insert(
        EXTRA_AI_PROFILE_UPDATED_AT.to_string(),
        Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    );
}

pub fn resolve_profile_ai_update_meta(
    data_dir: &Path,
    id: &str,
    profile: &CharacterProfileData,
) -> (bool, Option<String>) {
    if let Some(at) = profile
        .extra
        .get(EXTRA_AI_PROFILE_UPDATED_AT)
        .filter(|s| !s.trim().is_empty())
    {
        return (true, Some(at.clone()));
    }
    if !profile_is_structured(profile) {
        return (false, None);
    }
    let path = persona::personas_dir(data_dir).join(format!("{id}.json"));
    if let Ok(meta) = std::fs::metadata(&path) {
        if let Ok(modified) = meta.modified() {
            let dt: chrono::DateTime<Local> = modified.into();
            return (true, Some(dt.format("%Y-%m-%d %H:%M:%S").to_string()));
        }
    }
    (true, None)
}

pub fn persona_reference_path(data_dir: &Path, id: &str) -> std::path::PathBuf {
    persona::personas_dir(data_dir).join(format!("{id}.reference.md"))
}

pub fn save_persona_reference(data_dir: &Path, id: &str, text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }
    let path = persona_reference_path(data_dir, id);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())
}

pub fn load_persona_reference(data_dir: &Path, id: &str) -> Option<String> {
    let path = persona_reference_path(data_dir, id);
    std::fs::read_to_string(&path).ok().filter(|s| !s.trim().is_empty())
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
    mut profile: CharacterProfileData,
    skill_md: &str,
    name_hint: Option<&str>,
    source_hint: Option<&str>,
    mark_ai_updated: bool,
) -> Result<PersonaMeta, String> {
    let skill = skill_md.trim();
    if skill.is_empty() {
        return Err("Skill 文档为空".into());
    }
    if mark_ai_updated && profile_is_structured(&profile) {
        stamp_ai_profile_updated(&mut profile);
    }
    persona::save_persona_file(data_dir, id, skill)?;
    persona::save_persona_profile(data_dir, id, &profile)?;

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
        "AI 返回的 JSON 不完整（可能被截断）: {detail}。\
         已自动尝试压缩重试；若仍失败，请更换输出上限更高的思考模型，或等待供应商配额重置后再试。"
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
            '}' | ']' if !in_string && stack.last() == Some(&ch) => {
                stack.pop();
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

fn extract_json_object(s: &str) -> String {
    let s = s.trim();
    if let (Some(start), Some(end)) = (s.find('{'), s.rfind('}')) {
        if end > start {
            return s[start..=end].to_string();
        }
    }
    s.to_string()
}

fn extract_json_str(s: &str) -> String {
    extract_json_object(s)
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
    fn preprocess_wiki_uses_minimal_tiers() {
        let tiers = preprocess_attempt_tiers(true, 20_000);
        assert!(tiers.len() >= 2);
        assert_eq!(tiers[0].0, PreprocessMode::Minimal);
    }

    #[test]
    fn wiki_reference_local_profile() {
        let text = r#"# 角色：柴郡
来源：碧蓝航线 BWIKI

## 角色信息
身份：新晋猫女仆
性格：娇俏、粘人、爱撒娇
关键词：蹭蹭、摸摸、旦那樣

## 角色设定
不知为何穿着女仆装的皇家重巡洋舰。
明明没有下命令也会有时自顾自地打扫起值班室。

## 舰船台词（原文，共 50 条，已抽样 3 条）
- 呼啊-感觉像是睡了很久醒过来一样呢~
- 亲~爱~的~！嘿，我抱！
"#;
        let profile = try_profile_from_wiki_reference(text, "柴郡", "碧蓝航线").unwrap();
        assert_eq!(profile.name, "柴郡");
        assert!(profile.personality.iter().any(|p| p.contains("娇俏")));
        assert!(profile.introduction.contains("女仆装"));
        assert!(profile.speech_style.contains("关键词") || profile.speech_style.contains("亲昵"));
        assert_eq!(profile.sample_lines.len(), 2);
    }

    #[test]
    fn preprocess_wiki_uses_compact_tiers() {
        let tiers = preprocess_attempt_tiers(true, 20_000);
        assert!(tiers.iter().any(|(m, _)| *m == PreprocessMode::Compact));
        assert!(tiers.iter().any(|(_, cap)| *cap == MAX_REFERENCE_CHARS_ULTRA));
    }

    #[test]
    fn wiki_reference_messy_info_fields() {
        let text = r#"# 角色：33
来源：碧蓝航线 BWIKI

## 角色信息
身份：看板娘 性格 三无（但有点腹黑） 关键词 网站维护
性格：三无（但有点腹黑） 关键词 网站维护 持有物 小电视发饰

## 舰船台词（原文，共 2 条，已抽样 2 条）
- 我是33，22的妹妹，平时可能会吐槽。
- 登陆成功，请触摸33执行权限认证......认证成功，指挥官，欢迎回来。
"#;
        let profile = try_profile_from_wiki_reference(text, "33", "碧蓝航线 BWIKI").unwrap();
        assert!(profile.personality.iter().any(|p| p.contains("三无")));
        assert!(profile_is_structured(&profile));
    }

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
