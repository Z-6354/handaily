//! 台词智能导入：从混杂文本中提取台词，保留原文措辞

use std::collections::HashSet;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::pet::models::{self, PetRemarkLine};
use crate::state::AppState;

const STEP_TOTAL: u32 = 3;
const CHUNK_MAX: usize = 6000;

#[derive(Debug, Clone, Serialize)]
pub struct PetLinesImportProgressEvent {
    pub step: String,
    pub message: String,
    pub step_index: u32,
    pub step_total: u32,
}

fn emit_progress(app: &AppHandle, step: &str, message: &str, step_index: u32, step_total: u32) {
    let _ = app.emit(
        "pet-lines-import-progress",
        PetLinesImportProgressEvent {
            step: step.to_string(),
            message: message.to_string(),
            step_index,
            step_total,
        },
    );
}

pub(crate) fn emit_lines_progress(
    app: &AppHandle,
    step: &str,
    message: &str,
    step_index: u32,
    step_total: u32,
) {
    emit_progress(app, step, message, step_index, step_total);
}

pub async fn ai_import_lines(
    app: &AppHandle,
    st: &AppState,
    model_id: &str,
    raw_text: &str,
) -> Result<Vec<PetRemarkLine>, String> {
    let raw = raw_text.trim();
    if raw.is_empty() {
        return Err("请先粘贴要导入的文本".into());
    }

    emit_progress(app, "preprocess", "正在预处理文本…", 1, STEP_TOTAL);
    let local = local_extract_lines(raw);

    let data_dir = st.data_dir();
    let anim_meta = {
        let db = st.lock_db().map_err(|e| e.to_string())?;
        models::read_animation_meta(data_dir, &db, model_id)
    };

    emit_progress(app, "extract", "正在清洗提取台词…", 2, STEP_TOTAL);

    let ai_ready = {
        let db = st.lock_db().map_err(|e| e.to_string())?;
        let config = crate::ai::AiConfig::load(&db, data_dir);
        let catalog = crate::ai::load_catalog(data_dir);
        crate::ai::is_text_ai_ready(&config, &catalog, &st.vault, &db)
    };

    let mut extracted = if ai_ready {
        match extract_with_ai(app, st, raw, &anim_meta, 2, STEP_TOTAL).await {
            Ok(lines) if !lines.is_empty() => lines,
            Ok(_) => {
                if local.is_empty() {
                    return Err("AI 未能从文本中提取到台词".into());
                }
                local.clone()
            }
            Err(e) => {
                if local.is_empty() {
                    return Err(e);
                }
                local.clone()
            }
        }
    } else if !local.is_empty() {
        local.clone()
    } else {
        return Err(
            "未配置 AI 且无法从文本中解析出台词；请配置 AI 后使用智能导入，或使用 JSON / 逐行格式"
                .into(),
        );
    };

    if ai_ready && !local.is_empty() {
        extracted = merge_line_lists(extracted, local);
    }

    extracted = dedupe_lines(extracted);
    emit_progress(
        app,
        "validate",
        &format!("正在校验台词完整性（共 {} 条）…", extracted.len()),
        3,
        STEP_TOTAL,
    );

    if extracted.is_empty() {
        return Err("未能提取到有效台词".into());
    }
    Ok(extracted)
}

/// Wiki 爬取后的 AI 清洗：跳过「预处理」，使用自定义进度步骤。
pub async fn ai_clean_import_text(
    app: &AppHandle,
    st: &AppState,
    model_id: &str,
    raw_text: &str,
    extract_step: u32,
    validate_step: u32,
    step_total: u32,
) -> Result<Vec<PetRemarkLine>, String> {
    let raw = raw_text.trim();
    if raw.is_empty() {
        return Err("页面文本为空".into());
    }

    let local = local_extract_lines(raw);
    let data_dir = st.data_dir();
    let anim_meta = {
        let db = st.lock_db().map_err(|e| e.to_string())?;
        models::read_animation_meta(data_dir, &db, model_id)
    };

    let ai_ready = {
        let db = st.lock_db().map_err(|e| e.to_string())?;
        let config = crate::ai::AiConfig::load(&db, data_dir);
        let catalog = crate::ai::load_catalog(data_dir);
        crate::ai::is_text_ai_ready(&config, &catalog, &st.vault, &db)
    };

    let mut extracted = if ai_ready {
        match extract_with_ai(app, st, raw, &anim_meta, extract_step, step_total).await {
            Ok(lines) if !lines.is_empty() => lines,
            Ok(_) => {
                if local.is_empty() {
                    return Err("AI 未能从页面文本中提取到台词".into());
                }
                local.clone()
            }
            Err(e) => {
                if local.is_empty() {
                    return Err(e);
                }
                local.clone()
            }
        }
    } else if !local.is_empty() {
        local.clone()
    } else {
        return Err("未配置 AI 且无法从页面文本中解析出台词".into());
    };

    if ai_ready && !local.is_empty() {
        extracted = merge_line_lists(extracted, local);
    }

    extracted = dedupe_lines(extracted);
    emit_progress(
        app,
        "validate",
        &format!("正在校验台词完整性（共 {} 条）…", extracted.len()),
        validate_step,
        step_total,
    );

    if extracted.is_empty() {
        return Err("未能提取到有效台词".into());
    }
    Ok(extracted)
}

async fn extract_with_ai(
    app: &AppHandle,
    st: &AppState,
    raw: &str,
    anim_meta: &models::PetAnimationMeta,
    extract_step: u32,
    step_total: u32,
) -> Result<Vec<PetRemarkLine>, String> {
    let data_dir = st.data_dir();
    let animations = if anim_meta.animations.is_empty() {
        "（暂无，animation 一律填 null）".to_string()
    } else {
        anim_meta.animations.join("、")
    };

    let chunks = split_text_chunks(raw);
    let total = chunks.len();
    let mut all = Vec::new();

    for (idx, chunk) in chunks.iter().enumerate() {
        if total > 1 {
            emit_progress(
                app,
                "extract",
                &format!("正在清洗提取台词（第 {}/{} 段）…", idx + 1, total),
                extract_step,
                step_total,
            );
        }

        let prompt = crate::prompts::render(
            data_dir,
            "pet-lines-import",
            &[
                ("animations", &animations),
                ("raw_chunk", chunk),
            ],
        );

        let prep = {
            let db = st.lock_db().map_err(|e| e.to_string())?;
            let config = crate::ai::AiConfig::load(&db, data_dir);
            let catalog = crate::ai::load_catalog(data_dir);
            crate::ai::PreparedTextChat::prepare(
                &config,
                &catalog,
                &st.vault,
                &db,
                data_dir,
                prompt,
            )
            .map_err(|e| e.to_string())?
        };

        let prep = prep.ok_or_else(|| "未配置 AI 或密钥不可用".to_string())?;
        let ai_raw = prep.run_async().await.map_err(|e| e.to_string())?;
        let lines = parse_ai_lines(&ai_raw, &anim_meta.animations)?;
        all.extend(lines);
    }

    Ok(all)
}

fn parse_ai_lines(raw: &str, known_animations: &[String]) -> Result<Vec<PetRemarkLine>, String> {
    let json_str = crate::ai::json_util::extract_json_array(raw);
    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .or_else(|_| serde_json::from_str(raw))
        .map_err(|e| format!("AI 返回的 JSON 无法解析：{e}"))?;
    let arr = parsed
        .as_array()
        .cloned()
        .or_else(|| parsed.get("lines").and_then(|v| v.as_array()).cloned())
        .ok_or_else(|| "AI 返回格式无效，需要 JSON 数组".to_string())?;

    let has = |name: &str| {
        known_animations.is_empty() || known_animations.iter().any(|n| n == name)
    };

    let lines: Vec<PetRemarkLine> = arr
        .iter()
        .filter_map(|item| {
            let text = item.get("text")?.as_str()?.trim();
            if text.is_empty() || is_noise_line(text) {
                return None;
            }
            let animation = item
                .get("animation")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && has(s));
            Some(PetRemarkLine {
                text: text.to_string(),
                animation,
            })
        })
        .collect();

    if lines.is_empty() {
        return Err("AI 未返回可用台词".into());
    }
    Ok(lines)
}

pub fn local_extract_lines(raw: &str) -> Vec<PetRemarkLine> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(lines) = parse_json_lines(&parsed) {
            if !lines.is_empty() {
                return dedupe_lines(lines);
            }
        }
    }

    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for line in trimmed.lines() {
        for candidate in line_candidates(line) {
            if is_noise_line(&candidate.text) {
                continue;
            }
            if seen.insert(candidate.text.clone()) {
                out.push(candidate);
            }
        }
    }

    for quoted in extract_quoted_segments(trimmed) {
        if is_noise_line(&quoted) {
            continue;
        }
        if seen.insert(quoted.clone()) {
            out.push(PetRemarkLine {
                text: quoted,
                animation: None,
            });
        }
    }

    out
}

fn parse_json_lines(value: &serde_json::Value) -> Option<Vec<PetRemarkLine>> {
    let arr = value.as_array()?;
    let lines: Vec<PetRemarkLine> = arr
        .iter()
        .filter_map(|item| {
            if let Some(s) = item.as_str() {
                let text = s.trim();
                if text.is_empty() {
                    return None;
                }
                return Some(PetRemarkLine {
                    text: text.to_string(),
                    animation: None,
                });
            }
            let text = item.get("text")?.as_str()?.trim();
            if text.is_empty() {
                return None;
            }
            let animation = item
                .get("animation")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            Some(PetRemarkLine {
                text: text.to_string(),
                animation,
            })
        })
        .collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines)
    }
}

fn line_candidates(line: &str) -> Vec<PetRemarkLine> {
    let mut out = Vec::new();
    let trimmed = strip_line_prefix(line.trim());
    if trimmed.is_empty() {
        return out;
    }

    if let Some((animation, text)) = parse_action_prefix(trimmed) {
        let text = text.trim();
        if !text.is_empty() && !is_noise_line(text) {
            out.push(PetRemarkLine {
                text: text.to_string(),
                animation,
            });
            return out;
        }
    }

    if let Some(dialogue) = extract_speaker_dialogue(trimmed) {
        out.push(PetRemarkLine {
            text: dialogue,
            animation: None,
        });
        return out;
    }

    if !is_noise_line(trimmed) {
        out.push(PetRemarkLine {
            text: trimmed.to_string(),
            animation: None,
        });
    }
    out
}

fn strip_line_prefix(s: &str) -> &str {
    let mut t = s.trim();
    loop {
        let next = t
            .strip_prefix("- ")
            .or_else(|| t.strip_prefix("* "))
            .or_else(|| t.strip_prefix("• "))
            .or_else(|| t.strip_prefix("· "))
            .or_else(|| t.strip_prefix("+ "));
        match next {
            Some(rest) => t = rest.trim_start(),
            None => break,
        }
    }
    t = strip_numeric_prefix(t);
    t.trim()
}

fn strip_numeric_prefix(s: &str) -> &str {
    let t = s.trim_start();
    let mut i = 0;
    let bytes = t.as_bytes();
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 {
        let rest = &t[i..].trim_start();
        if rest.starts_with('.') || rest.starts_with('、') || rest.starts_with(')') {
            return rest[1..].trim_start();
        }
        if rest.starts_with(". ") {
            return rest[2..].trim_start();
        }
    }
    t
}

fn parse_action_prefix(line: &str) -> Option<(Option<String>, &str)> {
    let t = line.trim();
    if t.starts_with('[') {
        if let Some(end) = t.find(']') {
            let tag = t[1..end].trim();
            let rest = t[end + 1..].trim().trim_start_matches(['，', ',', '：', ':', ' ']);
            if !tag.is_empty() && !rest.is_empty() {
                return Some((Some(tag.to_string()), rest));
            }
        }
    }
    for prefix in ["动作:", "动作：", "animation:", "animation："] {
        if let Some(rest) = t.strip_prefix(prefix) {
            let rest = rest.trim();
            if let Some((anim, text)) = rest.split_once(['，', ',']) {
                let anim = anim.trim();
                let text = text.trim();
                if !anim.is_empty() && !text.is_empty() {
                    return Some((Some(anim.to_string()), text));
                }
            }
        }
    }
    None
}

fn extract_speaker_dialogue(line: &str) -> Option<String> {
    for sep in ['：', ':'] {
        if let Some((speaker, dialogue)) = line.split_once(sep) {
            let speaker = speaker.trim();
            let dialogue = dialogue.trim();
            if dialogue.len() >= 2
                && speaker.len() <= 24
                && !speaker.starts_with("http")
                && !is_noise_line(dialogue)
            {
                return Some(dialogue.to_string());
            }
        }
    }
    None
}

fn extract_quoted_segments(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = text.chars().collect();
    while i < chars.len() {
        let open = chars[i];
        let close = match open {
            '"' => Some('"'),
            '\'' => Some('\''),
            '「' => Some('」'),
            '『' => Some('』'),
            '“' => Some('”'),
            '‘' => Some('’'),
            _ => None,
        };
        if let Some(cl) = close {
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != cl {
                i += 1;
            }
            if i > start {
                let s: String = chars[start..i].iter().collect();
                let s = s.trim();
                if s.len() >= 2 {
                    out.push(s.to_string());
                }
            }
            if i < chars.len() {
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    out
}

fn is_noise_line(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() || t.chars().count() < 2 {
        return true;
    }
    if t.starts_with("http://") || t.starts_with("https://") {
        return true;
    }
    if t.starts_with('#') && t.chars().count() < 40 {
        return true;
    }
    let alnum = t.chars().filter(|c| !c.is_whitespace()).count();
    if alnum == 0 {
        return true;
    }
    false
}

fn split_text_chunks(raw: &str) -> Vec<String> {
    if raw.len() <= CHUNK_MAX {
        return vec![raw.to_string()];
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < raw.len() {
        let mut end = (start + CHUNK_MAX).min(raw.len());
        if end < raw.len() {
            if let Some(rel) = raw[start..end].rfind("\n\n") {
                end = start + rel + 2;
            } else if let Some(rel) = raw[start..end].rfind('\n') {
                end = start + rel + 1;
            }
        }
        chunks.push(raw[start..end].to_string());
        start = end;
    }
    chunks
}

pub(crate) fn dedupe_lines(lines: Vec<PetRemarkLine>) -> Vec<PetRemarkLine> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for line in lines {
        if line.text.trim().is_empty() {
            continue;
        }
        if seen.insert(line.text.clone()) {
            out.push(line);
        }
    }
    out
}

fn merge_line_lists(primary: Vec<PetRemarkLine>, supplement: Vec<PetRemarkLine>) -> Vec<PetRemarkLine> {
    let mut seen: HashSet<String> = primary.iter().map(|l| l.text.clone()).collect();
    let mut out = primary;
    for line in supplement {
        if seen.insert(line.text.clone()) {
            out.push(line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_extract_speaker_and_quotes() {
        let raw = r#"第一章
小寒：主人起床啦~
「今天也要加油哦」
- 动作:dance, 跳个舞给你看"#;
        let lines = local_extract_lines(raw);
        assert!(lines.iter().any(|l| l.text.contains("主人起床啦")));
        assert!(lines.iter().any(|l| l.text.contains("今天也要加油")));
        assert!(lines.iter().any(|l| l.text.contains("跳个舞")));
    }

    #[test]
    fn dedupe_preserves_order() {
        let lines = dedupe_lines(vec![
            PetRemarkLine { text: "a".into(), animation: None },
            PetRemarkLine { text: "b".into(), animation: None },
            PetRemarkLine { text: "a".into(), animation: None },
        ]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "a");
        assert_eq!(lines[1].text, "b");
    }
}
