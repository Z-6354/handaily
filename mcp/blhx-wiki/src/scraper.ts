const SKIP_LINE_KEYS = new Set(["extra", "drop_descrip"]);

const PERSONA_SECTION_IDS = [
  "情人节礼物",
  "舰船台词",
  "角色设定",
  "角色剧情卡（补充）",
  "角色剧情卡",
  "相关解释",
];

const CHARACTER_INFO_FIELDS = [
  "身份",
  "性格",
  "关键词",
  "持有物",
  "发色",
  "瞳色",
  "萌点",
];

const NOISE_LINE_MARKERS = [
  "配装推荐",
  "通用配装",
  "T0.jpg",
  "技能数据",
  "强度评价",
  "Skillicon",
  "skillicon",
];

export function decodeHtmlEntities(s: string): string {
  return s
    .replace(/&nbsp;/g, " ")
    .replace(/&#160;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'");
}

export function stripHtmlTags(s: string): string {
  let out = "";
  let inTag = false;
  for (const c of s) {
    if (c === "<") inTag = true;
    else if (c === ">") inTag = false;
    else if (!inTag) out += c;
  }
  return decodeHtmlEntities(out);
}

export function insertHtmlBreaks(html: string): string {
  let s = html
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<\/tr>/gi, "</tr>\n")
    .replace(/<\/td>/gi, "</td>\n")
    .replace(/<\/p>/gi, "</p>\n")
    .replace(/<\/li>/gi, "</li>\n");
  return s;
}

export function stripHtmlToText(html: string): string {
  const noScript = removeTagBlocks(html, "script");
  const noStyle = removeTagBlocks(noScript, "style");
  const withBreaks = insertHtmlBreaks(noStyle);
  return normalizeText(stripHtmlTags(withBreaks));
}

function removeTagBlocks(html: string, tag: string): string {
  const open = `<${tag}`;
  const close = `</${tag}>`;
  let out = "";
  let rest = html;
  while (true) {
    const start = rest.indexOf(open);
    if (start < 0) {
      out += rest;
      break;
    }
    out += rest.slice(0, start);
    const end = rest.slice(start).indexOf(close);
    if (end < 0) break;
    rest = rest.slice(start + end + close.length);
  }
  return out;
}

export function normalizeText(s: string): string {
  return s.split(/\s+/).filter(Boolean).join(" ").trim();
}

export function extractWikiSection(html: string, heading: string): string | null {
  const start =
    html.indexOf(`id="${heading}"`) >= 0
      ? html.indexOf(`id="${heading}"`)
      : html.indexOf(`>${heading}</span>`);
  if (start < 0) return null;
  const tail = html.slice(start);
  const contentStartMatch = tail.match(/<\/h[234]>/);
  const contentStart = contentStartMatch
    ? (contentStartMatch.index ?? 0) + contentStartMatch[0].length
    : 0;
  const content = tail.slice(contentStart);
  const relEnd = content.search(/<h[234]/);
  const end = relEnd >= 0 ? relEnd : content.length;
  return tail.slice(contentStart, contentStart + end);
}

export function parseCatalogEntries(html: string): import("./types.js").CatalogEntry[] {
  const cardArea = html.includes('id="CardSelectTr"')
    ? html.slice(html.indexOf('id="CardSelectTr"'))
    : html;
  const entries: import("./types.js").CatalogEntry[] = [];
  const cardRegex =
    /<div class="jntj-1 divsort"([^>]*)>([\s\S]*?)<\/div>\s*<span class="jntj-4">([\s\S]*?)<\/span><\/div>/g;

  for (const match of cardArea.matchAll(cardRegex)) {
    const attrs = match[1] ?? "";
    const body = match[2] ?? "";
    const nameBlock = match[3] ?? "";

    const titleMatch = nameBlock.match(/title="([^"]+)"/);
    const hrefMatch = nameBlock.match(/href="\/blhx\/([^"#?]+)"/);
    if (!titleMatch || !hrefMatch) continue;

    const wikiTitle = titleMatch[1].trim();
    const wikiPath = decodeURIComponent(hrefMatch[1]);
    const avatarMatch = body.match(/alt="([^"]+?)头像\.jpg"/);
    const avatarUrl =
      body.match(/src="(https:\/\/patchwiki\.biligame\.com[^"]+)"/)?.[1] ?? null;

    const param2 = attrs.match(/data-param2="([^"]*)"/)?.[1] ?? null;
    const param3 = attrs.match(/data-param3="([^"]*)"/)?.[1] ?? null;
    const param1 = attrs.match(/data-param1="([^"]*)"/)?.[1] ?? null;
    const shipType = param1?.split(",")[2]?.trim() || null;

    const aliases = extractAliases(nameBlock, wikiTitle);

    entries.push({
      wikiTitle,
      wikiPath,
      displayName: wikiTitle,
      aliases,
      avatarUrl,
      rarity: param2 || null,
      faction: param3 || null,
      shipType,
    });
  }

  const seen = new Set<string>();
  return entries.filter((e) => {
    if (seen.has(e.wikiTitle)) return false;
    seen.add(e.wikiTitle);
    return true;
  });
}

function extractAliases(nameBlock: string, primary: string): string[] {
  const text = stripHtmlTags(nameBlock);
  const parts = text
    .split(/\s+/)
    .map((p) => p.trim())
    .filter((p) => p && p !== primary);
  return [...new Set(parts)];
}

export function parseShipPage(
  html: string,
  wikiTitle: string,
  wikiUrl: string
): import("./types.js").ShipRecord {
  const displayName = guessDisplayName(html, wikiTitle);
  const aliases = extractPageAliases(html, displayName);
  const characterInfo = extractCharacterInfo(html);
  const sections = extractPersonaSections(html);
  const lines = extractShipLines(html);
  const assets = extractAssets(html, displayName);
  const cv = extractCv(html);
  const personaReference = buildPersonaReference({
    wikiTitle,
    wikiUrl,
    displayName,
    characterInfo,
    sections,
    lines,
  });

  return {
    wikiTitle,
    wikiUrl,
    displayName,
    aliases,
    rarity: null,
    faction: null,
    shipType: null,
    cv,
    characterInfo,
    sections,
    lines,
    assets,
    personaReference,
    fetchedAt: new Date().toISOString(),
    htmlHash: simpleHash(html),
  };
}

function guessDisplayName(html: string, fallback: string): string {
  const title = html.match(/<title>([^<]+)<\/title>/i)?.[1];
  if (title) {
    const name = title.split(/[-–|_]/)[0]?.trim();
    if (name && name !== "WIKI" && !name.includes("BWIKI")) return name;
  }
  return fallback;
}

function extractPageAliases(html: string, primary: string): string[] {
  const info = extractCharacterInfo(html);
  const aliasField = info.find((f) => f.field === "原名" || f.field === "别号");
  if (!aliasField) return [];
  return aliasField.value
    .split(/[,，、/]/)
    .map((s) => s.trim())
    .filter((s) => s && s !== primary);
}

function extractCharacterInfo(html: string): import("./types.js").CharacterInfoField[] {
  const section =
    extractWikiSection(html, "舰船信息") ?? html.slice(0, 20_000);
  const marker = section.indexOf("角色信息");
  if (marker < 0) return [];
  const block = section.slice(marker);
  const endMarkers = ["强度评价", "技能数据", "立绘"];
  let end = block.length;
  for (const m of endMarkers) {
    const idx = block.indexOf(m);
    if (idx > 0) end = Math.min(end, idx);
  }
  const plain = stripHtmlToText(block.slice(0, end));
  const rows: import("./types.js").CharacterInfoField[] = [];
  for (const field of CHARACTER_INFO_FIELDS) {
    const value = extractTableFieldValue(plain, field);
    if (value) rows.push({ field, value });
  }
  return rows;
}

function extractTableFieldValue(text: string, field: string): string | null {
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    for (const marker of [`**${field}**`, field]) {
      const idx = trimmed.indexOf(marker);
      if (idx >= 0) {
        const after = trimmed
          .slice(idx + marker.length)
          .replace(/^[\s*|：:]+/, "")
          .split("|")[0]
          ?.trim();
        if (after) return truncate(after.replace(/\[[^\]]*$/, "").trim(), 400);
      }
    }
  }
  const flat = text.split(/\s+/).join(" ");
  const needle = `${field} `;
  const idx = flat.indexOf(needle);
  if (idx < 0) return null;
  let tail = flat.slice(idx + field.length).replace(/^[：:\s|]+/, "");
  for (const other of CHARACTER_INFO_FIELDS) {
    if (other === field) continue;
    const p = tail.indexOf(` ${other}`);
    if (p > 0) tail = tail.slice(0, p);
  }
  const cleaned = tail.replace(/\[[^\]]*$/, "").trim();
  return cleaned ? truncate(cleaned, 400) : null;
}

function extractPersonaSections(html: string): import("./types.js").ShipSection[] {
  const sections: import("./types.js").ShipSection[] = [];
  for (const id of PERSONA_SECTION_IDS) {
    const sectionHtml = extractWikiSection(html, id);
    if (!sectionHtml) continue;
    const text = cleanSectionText(stripHtmlToText(sectionHtml));
    if (text.length > 20) sections.push({ id, title: id, text: truncate(text, 6000) });
  }
  return sections;
}

function cleanSectionText(text: string): string {
  return text
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean)
    .filter((line) => !NOISE_LINE_MARKERS.some((n) => line.includes(n)))
    .map((line) => {
      const idx = line.indexOf("http");
      return idx >= 0 ? line.slice(0, idx).trim() : line;
    })
    .filter((line) => line.length >= 2)
    .join("\n");
}

export function extractShipLines(html: string): import("./types.js").ShipLine[] {
  const section = extractWikiSection(html, "舰船台词") ?? html;
  const lines: import("./types.js").ShipLine[] = [];
  const rowRegex =
    /<tr[^>]*data-key="([^"]+)"[^>]*>[\s\S]*?<th[^>]*>([\s\S]*?)<\/th>[\s\S]*?<td[^>]*>([\s\S]*?)<\/td>[\s\S]*?<\/tr>/g;

  for (const match of section.matchAll(rowRegex)) {
    const key = match[1]?.trim();
    if (!key || SKIP_LINE_KEYS.has(key)) continue;
    const label = normalizeText(stripHtmlTags(match[2] ?? ""));
    const cell = match[3] ?? "";
    const text =
      normalizeText(
        stripHtmlTags(cell.match(/class="ship_word_line"[^>]*>([\s\S]*?)<\/p>/)?.[1] ?? "")
      ) || normalizeText(stripHtmlTags(cell));
    if (!text || text.length < 2) continue;
    const lang =
      cell.match(/data-lang="([^"]+)"/)?.[1] ??
      cell.match(/class="ship_word_line"[^>]*data-lang="([^"]+)"/)?.[1] ??
      "zh";
    const audioUrl =
      cell.match(/href="(https:\/\/patchwiki\.biligame\.com[^"]+\.(?:mp3|ogg|wav))"/i)?.[1] ??
      null;
    lines.push({ key, label: label || null, lang, text, audioUrl });
  }

  if (lines.length === 0) {
    for (const block of iterShipWordBlocks(section)) {
      const key = block.match(/data-key="([^"]+)"/)?.[1];
      if (!key || SKIP_LINE_KEYS.has(key)) continue;
      const text = normalizeText(
        stripHtmlTags(block.match(/class="ship_word_line"[^>]*>([\s\S]*?)<\/p>/)?.[1] ?? "")
      );
      if (text.length >= 2) {
        lines.push({
          key,
          label: null,
          lang: "zh",
          text,
          audioUrl:
            block.match(/href="(https:\/\/patchwiki\.biligame\.com[^"]+\.(?:mp3|ogg|wav))"/i)?.[1] ??
            null,
        });
      }
    }
  }

  return dedupeLines(lines);
}

function iterShipWordBlocks(section: string): string[] {
  const blocks: string[] = [];
  let search = section;
  while (true) {
    const pos = search.indexOf("ship_word_block");
    if (pos < 0) break;
    const after = search.slice(pos + "ship_word_block".length);
    const next = after.indexOf("ship_word_block");
    const end = next >= 0 ? pos + "ship_word_block".length + next : search.length;
    blocks.push(search.slice(pos, end));
    if (next < 0) break;
    search = search.slice(end);
  }
  return blocks;
}

function dedupeLines(lines: import("./types.js").ShipLine[]): import("./types.js").ShipLine[] {
  const seen = new Set<string>();
  return lines.filter((l) => {
    const k = `${l.key}|${l.lang}|${l.text}`;
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });
}

function extractAssets(html: string, displayName: string): import("./types.js").ShipAsset[] {
  const assets: import("./types.js").ShipAsset[] = [];
  const imgTagRegex = /<img\b[^>]*>/gi;
  for (const match of html.matchAll(imgTagRegex)) {
    const tag = match[0];
    const url = tag.match(/\bsrc="(https:\/\/patchwiki\.biligame\.com\/images\/blhx[^"]+)"/)?.[1];
    if (!url) continue;
    if (url.includes("ShipType-") || url.includes("Camplogo_") || url.includes("头像外框")) continue;
    const alt = tag.match(/\balt="([^"]*)"/)?.[1]?.trim() ?? "";
    const name = alt || decodeURIComponent(url.split("/").pop() ?? "image");
    const kind = classifyAsset(name, url);
    if (kind === "other" && !name.includes(displayName.slice(0, 2)) && !url.includes(encodeURIComponent(displayName.slice(0, 2)))) {
      continue;
    }
    assets.push({ kind, name, url });
  }
  return dedupeAssets(assets);
}

function classifyAsset(name: string, url: string): import("./types.js").ShipAsset["kind"] {
  const n = `${name} ${url}`;
  if (n.includes("头像")) return "avatar";
  if (/Q版|q版/.test(n)) return "chibi";
  if (n.includes("换装")) return "skin";
  if (n.includes("立绘")) return "illustration";
  return "other";
}

function dedupeAssets(assets: import("./types.js").ShipAsset[]): import("./types.js").ShipAsset[] {
  const seen = new Set<string>();
  return assets.filter((a) => {
    if (seen.has(a.url)) return false;
    seen.add(a.url);
    return true;
  });
}

function extractCv(html: string): string | null {
  const section = extractWikiSection(html, "舰船信息") ?? html.slice(0, 15_000);
  const cvSection = section.match(/CV[\s\S]{0,800}/i)?.[0];
  if (!cvSection) return extractTableFieldValue(stripHtmlToText(section), "CV");
  const text = stripHtmlToText(cvSection).replace(/^CV\s*[：:]?\s*/i, "").trim();
  return text ? truncate(text.split("\n")[0] ?? text, 120) : null;
}

function buildPersonaReference(args: {
  wikiTitle: string;
  wikiUrl: string;
  displayName: string;
  characterInfo: import("./types.js").CharacterInfoField[];
  sections: import("./types.js").ShipSection[];
  lines: import("./types.js").ShipLine[];
}): string {
  const parts: string[] = [
    `# 角色：${args.displayName}`,
    "来源：碧蓝航线 BWIKI",
    `Wiki：${args.wikiUrl}`,
  ];
  if (args.characterInfo.length) {
    parts.push(
      "## 角色信息\n" +
        args.characterInfo.map((f) => `${f.field}：${f.value}`).join("\n")
    );
  }
  for (const section of args.sections) {
    if (section.id === "舰船台词") continue;
    parts.push(`## ${section.title}\n${truncate(section.text, 900)}`);
  }
  const dialogue = sampleLines(args.lines, 12);
  if (dialogue.length) {
    parts.push(
      `## 舰船台词（原文，共 ${args.lines.length} 条，已抽样 ${dialogue.length} 条）`
    );
    for (const line of dialogue) parts.push(`- ${line.text}`);
  }
  return truncate(parts.join("\n\n"), 5500);
}

function sampleLines(
  lines: import("./types.js").ShipLine[],
  max: number
): import("./types.js").ShipLine[] {
  if (lines.length <= max) return lines;
  const head = Math.min(8, max);
  const picked = lines.slice(0, head);
  const rest = lines.slice(head);
  const step = Math.max(1, Math.floor(rest.length / (max - head)));
  for (let i = 0; i < rest.length && picked.length < max; i += step) {
    picked.push(rest[i]!);
  }
  return picked;
}

function truncate(s: string, max: number): string {
  if (s.length <= max) return s;
  return s.slice(0, max) + "…";
}

function simpleHash(s: string): string {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (Math.imul(31, h) + s.charCodeAt(i)) | 0;
  return (h >>> 0).toString(16);
}

export function slugifyPersonaId(name: string): string {
  return name
    .toLowerCase()
    .replace(/[·.\s()（）μ]/g, "_")
    .replace(/[^a-z0-9_\u4e00-\u9fff-]/g, "")
    .replace(/_+/g, "_")
    .replace(/^_|_$/g, "")
    .slice(0, 48) || "ship";
}
