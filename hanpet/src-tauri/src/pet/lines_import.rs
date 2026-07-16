//! [live2d-only] 台词本地解析导入

use std::collections::HashSet;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::pet::models::PetRemarkLine;

#[derive(Debug, Clone, Serialize)]
pub struct PetLinesImportProgressEvent {
    pub step: String,
    pub message: String,
    pub step_index: u32,
    pub step_total: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetWikiBulkImportStartResult {
    pub started: bool,
    pub already_running: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetWikiBulkImportProgress {
    pub phase: String,
    pub index: u32,
    pub total: u32,
    pub model_id: String,
    pub model_name: String,
    pub message: String,
    pub lines_imported: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub skipped: u32,
    pub updated_at_ms: i64,
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

pub fn emit_wiki_bulk_import_progress(app: &AppHandle, mut payload: PetWikiBulkImportProgress) {
    payload.updated_at_ms = chrono::Utc::now().timestamp_millis();
    if let Some(rt) = app.try_state::<crate::pet::PetRuntimeState>() {
        if let Ok(mut guard) = rt.wiki_bulk_last_progress.lock() {
            *guard = Some(payload.clone());
        }
    }
    let _ = app.emit("pet-wiki-bulk-import-progress", payload);
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
        if let Some(stripped) = rest.strip_prefix(". ") {
            return stripped.trim_start();
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
