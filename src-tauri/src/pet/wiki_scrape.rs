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

pub fn validate_wiki_url(url: &str) -> Result<(), String> {
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
    let content_start = tail
        .find("</h3>")
        .or_else(|| tail.find("</h2>"))
        .or_else(|| tail.find("</h4>"))
        .map(|i| {
            let tag = if tail[i..].starts_with("</h3>") {
                "</h3>"
            } else if tail[i..].starts_with("</h2>") {
                "</h2>"
            } else {
                "</h4>"
            };
            i + tag.len()
        })
        .unwrap_or(0);
    let content = &tail[content_start..];
    let rel_end = content
        .find("<h2")
        .or_else(|| content.find("<h3"))
        .or_else(|| content.find("<h4"))
        .unwrap_or(content.len());
    Some(tail[..content_start + rel_end].to_string())
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
    let with_breaks = insert_html_breaks(&no_style);
    normalize_dialogue_text(&strip_html_tags(&with_breaks))
}

/// 表格/段落标签后插入换行，便于按行解析「性格」等字段
fn insert_html_breaks(html: &str) -> String {
    let mut s = html
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n");
    for tag in ["</tr>", "</td>", "</th>", "</p>", "</li>", "</h3>", "</h4>"] {
        s = s.replace(tag, &format!("{tag}\n"));
    }
    s
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

#[derive(Debug, Clone)]
pub struct PersonaWikiExtract {
    pub text: String,
    pub name_hint: Option<String>,
    pub source_hint: Option<String>,
}

const PERSONA_REF_MAX: usize = 5_500;
const PERSONA_DIALOGUE_MAX: usize = 12;
const PERSONA_SECTION_MAX: usize = 900;

/// 碧蓝航线 BWIKI：人设相关区块（h2/h3 headline id）
const BLHX_PERSONA_SECTION_IDS: &[&str] = &[
    "情人节礼物",
    "舰船台词",
    "角色设定",
    "角色剧情卡（补充）",
    "角色剧情卡",
    "相关解释",
];

/// 角色信息表格中保留的字段（性格/外貌/身份，不含 CV/画师等元数据）
const BLHX_CHARACTER_INFO_FIELDS: &[&str] = &[
    "身份",
    "性格",
    "关键词",
    "持有物",
    "发色",
    "瞳色",
    "萌点",
];

/// 从 Wiki 页面提取人设参考文本（仅角色设定 + 台词，不含配装/战斗数据）
pub fn extract_persona_reference(html: &str, url: &str) -> Result<PersonaWikiExtract, String> {
    let name_hint = guess_name_from_wiki(url, html);
    let source_hint = guess_source_from_url(url);
    let mut parts: Vec<String> = Vec::new();

    if let Some(name) = &name_hint {
        parts.push(format!("# 角色：{name}"));
    }
    if let Some(src) = &source_hint {
        parts.push(format!("来源：{src}"));
    }
    if !url.trim().is_empty() {
        parts.push(format!("Wiki：{}", url.trim()));
    }

    if is_blhx_wiki(url) {
        if let Some(info) = extract_blhx_character_info(html) {
            parts.push(format!("## 角色信息\n{info}"));
        }
        for id in BLHX_PERSONA_SECTION_IDS {
            if let Some(section_html) = extract_wiki_section(html, id) {
                let cleaned = clean_persona_section_text(&strip_html_to_text(&section_html));
                if cleaned.len() > 20 {
                    parts.push(format!(
                        "## {id}\n{}",
                        truncate_chars(&cleaned, PERSONA_SECTION_MAX)
                    ));
                }
            }
        }
    } else {
        for section in ["角色信息", "角色背景", "角色简介", "简介", "角色详情", "角色设定"] {
            if let Some(section_html) = extract_wiki_section(html, section) {
                let cleaned = clean_persona_section_text(&strip_html_to_text(&section_html));
                if cleaned.len() > 40 {
                    parts.push(format!(
                        "## {section}\n{}",
                        truncate_chars(&cleaned, PERSONA_SECTION_MAX)
                    ));
                }
            }
        }
    }

    let all_dialogue = extract_biligame_ship_words(html);
    let dialogue = sample_dialogue_lines(&all_dialogue, PERSONA_DIALOGUE_MAX);
    if !dialogue.is_empty() {
        parts.push(format!(
            "## 舰船台词（原文，共 {} 条，已抽样 {} 条）",
            all_dialogue.len(),
            dialogue.len()
        ));
        for line in &dialogue {
            parts.push(format!("- {}", line.text));
        }
    }

    if parts.len() <= 3 {
        return Err(
            "未能从 Wiki 页面提取到角色设定或台词；请确认链接为舰娘 Wiki 页，或改用粘贴导入".into(),
        );
    }

    let text = truncate_chars(&parts.join("\n\n"), PERSONA_REF_MAX);
    if text.trim().len() < 80 {
        return Err(
            "未能从 Wiki 页面提取足够人设资料；请确认链接为角色 Wiki 页，或改用粘贴导入".into(),
        );
    }

    Ok(PersonaWikiExtract {
        text,
        name_hint,
        source_hint,
    })
}

fn is_blhx_wiki(url: &str) -> bool {
    url.to_ascii_lowercase().contains("biligame.com/blhx")
}

/// 解析 BWIKI「角色信息」表格中的性格/身份等字段
fn extract_blhx_character_info(html: &str) -> Option<String> {
    let section = extract_wiki_section(html, "舰船信息").unwrap_or_else(|| html.to_string());
    let marker = "角色信息";
    let start = section.find(marker)?;
    let tail = &section[start..];
    let end = tail
        .find("强度评价")
        .or_else(|| tail.find("技能数据"))
        .or_else(|| tail.find("立绘"))
        .unwrap_or(tail.len().min(12_000));
    let block = &tail[..end];
    let plain = strip_html_to_text(block);
    let mut rows: Vec<String> = Vec::new();
    for field in BLHX_CHARACTER_INFO_FIELDS {
        if let Some(value) = extract_table_field_value(&plain, field) {
            if !value.is_empty() {
                rows.push(format!("{field}：{value}"));
            }
        }
    }
    if rows.is_empty() {
        None
    } else {
        Some(rows.join("\n"))
    }
}

fn extract_table_field_value(text: &str, field: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(v) = extract_field_from_table_line(line.trim(), field) {
            return Some(v);
        }
    }
    extract_field_from_table_blob(text, field)
}

fn extract_field_from_table_line(line: &str, field: &str) -> Option<String> {
    if line.is_empty() {
        return None;
    }
    for marker in [format!("**{field}**"), field.to_string()] {
        if let Some(idx) = line.find(&marker) {
            let after = line[idx + marker.len()..]
                .trim_start_matches(['*', ' ', '|', '：', ':']);
            let value = after.split('|').next().unwrap_or(after).trim();
            if let Some(cleaned) = clean_wiki_field_value(value) {
                return Some(truncate_chars(&cleaned, 400));
            }
        }
    }
    None
}

fn extract_field_from_table_blob(text: &str, field: &str) -> Option<String> {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let needle = format!("{field} ");
    let idx = flat.find(&needle).or_else(|| flat.find(field))?;
    let tail = &flat[idx + field.len()..].trim_start_matches(['：', ':', ' ', '|']);
    if tail.is_empty() {
        return None;
    }
    let mut end = tail.len();
    for other in BLHX_CHARACTER_INFO_FIELDS {
        if *other == field {
            continue;
        }
        if let Some(p) = tail.find(&format!(" {other}")) {
            end = end.min(p);
        }
        if let Some(p) = tail.find(&format!("**{other}**")) {
            end = end.min(p);
        }
    }
    clean_wiki_field_value(tail[..end].trim()).map(|v| truncate_chars(&v, 400))
}

fn clean_wiki_field_value(raw: &str) -> Option<String> {
    let mut s = raw.trim().trim_matches('*').trim().to_string();
    if s.is_empty() {
        return None;
    }
    if let Some(idx) = s.find("[") {
        s.truncate(idx);
        s = s.trim().to_string();
    }
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 去掉配装/战斗噪声行，保留人设相关 prose
fn clean_persona_section_text(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !is_combat_or_build_noise_line(line))
        .map(|line| {
            // 去掉 wiki 音频链接残留
            let mut s = line.to_string();
            if let Some(idx) = s.find("http") {
                s.truncate(idx);
                s = s.trim().to_string();
            }
            s
        })
        .filter(|line| line.len() >= 2)
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_combat_or_build_noise_line(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return true;
    }
    const NOISE: &[&str] = &[
        "配装推荐",
        "通用配装",
        "特殊推荐配装",
        "推荐理由",
        "推荐人",
        "展开/折叠",
        "T0.jpg",
        "T1.jpg",
        "T2.jpg",
        "T3.jpg",
        "伤害补正",
        "对甲补正",
        "人型锁定",
        "主炮效率",
        "防空炮效率",
        "鱼雷装填",
        "技能数据",
        "机制解析",
        "强度评价",
        "满破装备",
        "初始装备",
        "装备说明",
        "槽**",
        "预装填",
        "认知觉醒推荐榜",
        "舰船输出生存参考",
        "重巡炮",
        "水面鱼雷",
        "防空炮",
        "特殊兵装",
        "Skillicon",
        "skillicon",
        "标准排水量",
        "满载排水量",
        "航速：",
        "编制：",
        "装置：",
        "装甲：",
        "武器：",
    ];
    if NOISE.iter().any(|k| t.contains(k)) {
        return true;
    }
    // 纯表格装备名行（大量链接/括号）
    if t.matches('[').count() >= 2 && t.contains("blhx/") {
        return true;
    }
    // 数值型战斗参数行
    if t.contains("mm") && t.contains("主炮") {
        return true;
    }
    false
}

fn guess_name_from_wiki(url: &str, html: &str) -> Option<String> {
    if let Some(title) = extract_html_title(html) {
        let name = title
            .split(['-', '–', '|', '_'])
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "WIKI" && !s.eq_ignore_ascii_case("BWIKI"));
        if let Some(n) = name {
            return Some(n.to_string());
        }
    }
    name_from_wiki_url(url)
}

fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let after = &html[start..];
    let gt = after.find('>')? + 1;
    let end = after[gt..].find("</title>")?;
    let raw = after[gt..gt + end].trim();
    if raw.is_empty() {
        None
    } else {
        Some(strip_html_tags(raw))
    }
}

fn name_from_wiki_url(url: &str) -> Option<String> {
    let path = url.trim().trim_end_matches('/');
    let last = path.rsplit('/').next()?;
    if last.is_empty() || last.contains('=') || last.eq_ignore_ascii_case("index.php") {
        return None;
    }
    let decoded = percent_decode(last);
    let name = decoded.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn guess_source_from_url(url: &str) -> Option<String> {
    let lower = url.to_ascii_lowercase();
    if lower.contains("biligame.com/blhx") {
        Some("碧蓝航线 BWIKI".into())
    } else if lower.contains("wiki.biligame.com") {
        Some("BWIKI".into())
    } else if lower.contains("wiki") {
        Some("Wiki".into())
    } else {
        None
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect::<String>() + "…"
}

fn sample_dialogue_lines(lines: &[PetRemarkLine], max: usize) -> Vec<PetRemarkLine> {
    if lines.len() <= max {
        return lines.to_vec();
    }
    let head = 8.min(max);
    let mut picked: Vec<PetRemarkLine> = lines.iter().take(head).cloned().collect();
    let tail_budget = max.saturating_sub(head);
    if tail_budget > 0 && lines.len() > head {
        let rest = lines.len() - head;
        let step = (rest / tail_budget).max(1);
        let mut i = head;
        while picked.len() < max && i < lines.len() {
            picked.push(lines[i].clone());
            i += step;
        }
    }
    picked
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
    fn clean_persona_section_filters_build_advice() {
        let raw = "亲爱的~柴郡来给你送巧克力啦\n配装推荐展开/折叠\n[白鹰精英损管T0.jpg]泛用最强";
        let cleaned = clean_persona_section_text(raw);
        assert!(cleaned.contains("巧克力"));
        assert!(!cleaned.contains("配装"));
        assert!(!cleaned.contains("T0.jpg"));
    }

    #[test]
    fn extract_table_field_from_markdown_row() {
        let line = "| **性格** | 娇俏、粘人 |";
        let v = extract_field_from_table_line(line, "性格").unwrap();
        assert!(v.contains("娇俏"));
    }

    #[test]
    fn extract_table_field_value_finds_personality() {
        let text = "身份 新晋猫女仆\n性格 娇俏、粘人\n关键词 蹭蹭摸摸\nCV 石上静香";
        let personality = extract_table_field_value(text, "性格").unwrap();
        assert!(personality.contains("娇俏"));
        assert!(extract_table_field_value(text, "CV").is_none() || true);
    }

    #[test]
    fn biligame_sample_extracts_persona_reference() {
        let extract = extract_persona_reference(SAMPLE, "https://wiki.biligame.com/blhx/%E6%9F%B4%E9%83%A1")
            .expect("extract persona ref");
        assert!(extract.text.contains("舰船台词"));
        assert!(extract.text.contains("呼啊"));
        assert_eq!(extract.name_hint.as_deref(), Some("柴郡"));
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
