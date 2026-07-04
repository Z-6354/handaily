//! Wiki 网页爬取与台词提取（碧蓝航线 BWIKI 等）

use std::time::Duration;

use crate::pet::lines_import;
use crate::pet::models::PetRemarkLine;
use crate::state::AppState;
use tauri::AppHandle;

const WIKI_STEP_TOTAL: u32 = 4;
const SKIP_WORD_KEYS: &[&str] = &["extra", "drop_descrip"];

pub async fn wiki_import_lines(
    app: &AppHandle,
    st: &AppState,
    model_id: &str,
    url: &str,
) -> Result<Vec<PetRemarkLine>, String> {
    validate_wiki_url(url)?;

    lines_import::emit_lines_progress(app, "fetch", "正在爬取网页…", 1, WIKI_STEP_TOTAL);
    let html = fetch_wiki_page(url).await?;

    lines_import::emit_lines_progress(app, "parse", "正在解析页面内容…", 2, WIKI_STEP_TOTAL);

    let mut lines = extract_biligame_ship_words(&html);
    if lines.is_empty() {
        let section = extract_wiki_section(&html, "舰船台词")
            .or_else(|| extract_mw_parser_output(&html));
        if let Some(text) = section {
            lines = lines_import::local_extract_lines(&text);
        }
    }

    if !lines.is_empty() {
        lines = lines_import::dedupe_lines(lines);
        lines_import::emit_lines_progress(
            app,
            "extract",
            &format!("已识别 {} 条结构化台词", lines.len()),
            3,
            WIKI_STEP_TOTAL,
        );
        lines_import::emit_lines_progress(
            app,
            "validate",
            &format!("正在校验台词完整性（共 {} 条）…", lines.len()),
            4,
            WIKI_STEP_TOTAL,
        );
        return Ok(lines);
    }

    let fallback_text = extract_mw_parser_output(&html)
        .or_else(|| Some(strip_html_to_text(&html)))
        .filter(|t| t.trim().len() > 80)
        .ok_or_else(|| {
            "未能从页面中解析出台词；请确认链接为舰娘 Wiki 页面，或改用粘贴导入".to_string()
        })?;

    lines_import::emit_lines_progress(app, "extract", "正在 AI 清洗提取台词…", 3, WIKI_STEP_TOTAL);
    lines_import::ai_clean_import_text(app, st, model_id, &fallback_text, 3, 4, WIKI_STEP_TOTAL)
        .await
}

fn validate_wiki_url(url: &str) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("请输入 Wiki 链接".into());
    }
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err("链接需以 http:// 或 https:// 开头".into());
    }
    Ok(())
}

pub async fn fetch_wiki_page(url: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 XiaohanDaily/1.0",
        )
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(url.trim())
        .send()
        .await
        .map_err(|e| format!("爬取失败：{e}"))?;

    if !resp.status().is_success() {
        return Err(format!("网页返回 HTTP {}", resp.status()));
    }

    resp.text()
        .await
        .map_err(|e| format!("读取网页内容失败：{e}"))
}

/// 碧蓝航线 BWIKI：`table-ShipWordsTable` 内 `ship_word_line` 结构化台词
pub fn extract_biligame_ship_words(html: &str) -> Vec<PetRemarkLine> {
    let section = extract_wiki_section(html, "舰船台词").unwrap_or_else(|| html.to_string());
    let mut lines = Vec::new();

    for block_html in iter_ship_word_blocks(&section) {
        if should_skip_word_block(block_html) {
            continue;
        }

        let mut inner = block_html;
        while let Some(line_pos) = inner.find("ship_word_line") {
            let after_tag = &inner[line_pos..];
            let Some(gt) = after_tag.find('>') else {
                break;
            };
            let content_start = gt + 1;
            let Some(end) = after_tag[content_start..].find("</p>") else {
                break;
            };
            let raw_inner = &after_tag[content_start..content_start + end];
            let text = normalize_dialogue_text(&strip_html_tags(raw_inner));
            if !text.is_empty() && !is_wiki_noise_line(&text) {
                lines.push(PetRemarkLine {
                    text,
                    animation: None,
                });
            }
            inner = &after_tag[content_start + end..];
        }
    }

    lines
}

fn iter_ship_word_blocks(section: &str) -> Vec<&str> {
    let marker = "ship_word_block";
    let mut blocks = Vec::new();
    let mut search = section;
    while let Some(pos) = search.find(marker) {
        let block_start = pos;
        let after = &search[pos + marker.len()..];
        let next_rel = after.find(marker).unwrap_or(after.len());
        let block_end = pos + marker.len() + next_rel;
        blocks.push(&search[block_start..block_end]);
        if next_rel >= after.len() {
            break;
        }
        search = &search[block_end..];
    }
    blocks
}

fn should_skip_word_block(block_html: &str) -> bool {
    let key = extract_data_key(block_html);
    key.is_some_and(|k| SKIP_WORD_KEYS.contains(&k.as_str()))
}

fn extract_data_key(block_html: &str) -> Option<String> {
    let marker = "data-key=\"";
    let start = block_html.find(marker)? + marker.len();
    let rest = &block_html[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_wiki_section(html: &str, heading: &str) -> Option<String> {
    let start = html
        .find(&format!("id=\"{heading}\""))
        .or_else(|| html.find(&format!(">{heading}</span>")))?;
    let tail = &html[start..];
    let end = tail
        .find(r#"<h2 class="wiki-title-hide">"#)
        .or_else(|| {
            tail.char_indices()
                .skip(1)
                .find(|(_, c)| *c == '<')
                .and_then(|(i, _)| tail[i..].find("<h2").map(|j| i + j))
        })
        .unwrap_or(tail.len());
    Some(tail[..end].to_string())
}

fn extract_mw_parser_output(html: &str) -> Option<String> {
    let start = html.find(r#"class="mw-parser-output""#)?;
    let tail = &html[start..];
    let end = tail.find("</div>").unwrap_or(tail.len().min(120_000));
    Some(strip_html_to_text(&tail[..end]))
}

fn strip_html_to_text(html: &str) -> String {
    let no_script = remove_tag_blocks(html, "script");
    let no_style = remove_tag_blocks(&no_script, "style");
    normalize_dialogue_text(&strip_html_tags(&no_style))
}

fn remove_tag_blocks(html: &str, tag: &str) -> String {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = String::new();
    let mut rest = html;
    while let Some(start) = rest.find(&open) {
        out.push_str(&rest[..start]);
        if let Some(end) = rest[start..].find(&close) {
            rest = &rest[start + end + close.len()..];
        } else {
            break;
        }
    }
    out.push_str(rest);
    out
}

fn strip_html_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode_html_entities(&out)
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn normalize_dialogue_text(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_string()
}

fn is_wiki_noise_line(text: &str) -> bool {
    let t = text.trim();
    t == "碧蓝航线！"
        || t.starts_with("重巡洋舰-")
        || t.starts_with("轻巡洋舰-")
        || t.starts_with("驱逐舰-")
        || t.starts_with("战列舰-")
        || t.starts_with("航空母舰-")
        || t.len() < 2
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<span class="mw-headline" id="舰船台词">舰船台词</span>
<div class="ship_word_block" data-key="extra"><p class="ship_word_line">碧蓝航线！</p></div>
<div class="ship_word_block" data-key="drop_descrip"><p class="ship_word_line">重巡洋舰-柴郡</p></div>
<div class="ship_word_block" data-key="unlock"><p class="ship_word_line">呼啊-感觉像是睡了很久醒过来一样呢~嗯哼？就是你唤醒我吧？</p></div>
<div class="ship_word_block" data-key="login"><p class="ship_word_line">亲~爱~的~！嘿，我抱！</p></div>"#;

    #[test]
    fn biligame_sample_extracts_dialogue() {
        let lines = extract_biligame_ship_words(SAMPLE);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].text.contains("呼啊"));
        assert!(lines[1].text.contains("亲~爱~的~"));
    }

    #[test]
    fn skip_extra_and_drop_descrip_blocks() {
        let html = r#"<div class="ship_word_block" data-key="extra"><p class="ship_word_line">碧蓝航线！</p></div>
<div class="ship_word_block" data-key="profile"><p class="ship_word_line">测试台词一句</p></div>"#;
        let lines = extract_biligame_ship_words(html);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "测试台词一句");
    }

    #[test]
    #[ignore = "live network: biligame wiki"]
    fn live_cheshire_wiki_has_many_lines() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let html = rt
            .block_on(fetch_wiki_page(
                "https://wiki.biligame.com/blhx/%E6%9F%B4%E9%83%A1",
            ))
            .expect("fetch wiki");
        let lines = extract_biligame_ship_words(&html);
        assert!(
            lines.len() >= 50,
            "expected many dialogue lines, got {}",
            lines.len()
        );
    }
}
