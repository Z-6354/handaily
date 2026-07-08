import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { pinyin } from "pinyin-pro";
import type { BlhxDatabase } from "./db.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ALIASES_PATH = path.resolve(__dirname, "../data/live2d-aliases.json");

/** 皮肤/变体后缀（先剥离） */
const SKIN_SUFFIX =
  /_(?:\d+|h|g|painting|idol|younv|summer|school|winter|swimsuit|wedding|newyear|cn|jp|en|super)$/i;

/** 舰种后缀（部分 live2d 包按舰种分文件夹，含 aijiangbb 这种无下划线形式） */
const CLASS_SUFFIX = /_(?:bb|cv|cl|dd|ca|bc|ss|ar|ae|cb|t|m|b|n)$/i;
const CLASS_SUFFIX_GLUE = /(?:bb|cv|cl|dd|ca|bc|ss|ar|ae|cb)$/i;

/** live2d 文件夹 slug → BWIKI display_name（与标准拼音不一致时手动维护） */
const LIVE2D_ALIASES: Record<string, string> = {
  aijiang: "埃吉尔",
  aosiben: "查尔斯·奥斯本",
  bangfeng: "邦克山",
  bolisi: "印第安纳波利斯",
  dafenqi: "大凤",
  dafen: "大凤",
};

function loadAliasMap(): Record<string, string> {
  const merged = { ...LIVE2D_ALIASES };
  if (fs.existsSync(ALIASES_PATH)) {
    try {
      const raw = JSON.parse(fs.readFileSync(ALIASES_PATH, "utf8")) as Record<string, string>;
      Object.assign(merged, raw);
    } catch {
      /* ignore invalid aliases file */
    }
  }
  return merged;
}

const HONORIFIC_SUFFIX = /(?:qinwang|gai|meta|alter|ii|iii|iv)$/i;

export interface Live2dFolder {
  folder: string;
  folderPath: string;
  baseSlug: string;
  skinSuffix: string;
  skelFile: string;
  hasSpine: boolean;
}

export interface ShipSlugIndex {
  wikiTitle: string;
  displayName: string;
  fullSlug: string;
  shortSlug: string;
  isMeta: boolean;
}

export interface Live2dMatch {
  folder: string;
  folderPath: string;
  skinLabel: string;
  baseSlug: string;
  wikiTitle: string;
  displayName: string;
  score: number;
  matchKind: string;
  ambiguous?: Array<{ displayName: string; wikiTitle: string; score: number }>;
}

export interface Live2dImportPlanItem {
  folder: string;
  folderPath: string;
  skinName: string;
  modelName: string;
  wikiTitle: string;
  displayName: string;
  score: number;
  characterId?: string;
  personaImported: boolean;
  alreadyImported: boolean;
  action: "import" | "skip_low_score" | "skip_no_persona" | "skip_exists";
}

export function defaultLive2dRoot(): string {
  const env = process.env.HANDAILY_LIVE2D_PATH?.trim();
  if (env) return path.resolve(env);
  return path.resolve(process.cwd(), "../../live2d");
}

export function toPinyinSlug(text: string): string {
  return pinyin(text, { toneType: "none", type: "array" })
    .join("")
    .replace(/[·・\s\-]/g, "")
    .replace(/[^a-z0-9]/gi, "")
    .toLowerCase();
}

export function stripSkinSuffix(folder: string): { base: string; skinSuffix: string } {
  const m = folder.match(SKIN_SUFFIX);
  if (!m) return { base: folder, skinSuffix: "" };
  return {
    base: folder.slice(0, m.index),
    skinSuffix: m[0].slice(1).toLowerCase(),
  };
}

export function stripClassSuffix(slug: string): string {
  const underscored = slug.replace(CLASS_SUFFIX, "");
  if (underscored !== slug) return underscored;
  const m = slug.match(new RegExp(`^(.{4,})${CLASS_SUFFIX_GLUE.source}$`, "i"));
  return m ? m[1] : slug;
}

export function shortSlug(full: string): string {
  if (full.length <= 4) return full;
  const trimmed = full.replace(HONORIFIC_SUFFIX, "");
  return trimmed.length >= 3 ? trimmed : full;
}

export function skinLabelFromSuffix(suffix: string, folder: string): string {
  if (!suffix) return "默认";
  if (/^\d+$/.test(suffix)) return `皮肤${suffix}`;
  const map: Record<string, string> = {
    h: "便服",
    g: "泳装",
    painting: "立绘",
    idol: "偶像",
    younv: "幼女",
    summer: "夏日",
    school: "学园",
    winter: "冬装",
    swimsuit: "泳装",
    wedding: "婚纱",
    newyear: "新年",
  };
  return map[suffix.toLowerCase()] ?? `变体_${suffix}`;
}

function levenshtein(a: string, b: string): number {
  const m = a.length;
  const n = b.length;
  const dp: number[][] = Array.from({ length: m + 1 }, (_, i) => [i]);
  for (let j = 1; j <= n; j++) dp[0][j] = j;
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      dp[i][j] =
        a[i - 1] === b[j - 1]
          ? dp[i - 1][j - 1]
          : 1 + Math.min(dp[i - 1][j], dp[i][j - 1], dp[i - 1][j - 1]);
    }
  }
  return dp[m][n];
}

function fuzzyRatio(a: string, b: string): number {
  if (!a || !b) return 0;
  const d = levenshtein(a, b);
  const max = Math.max(a.length, b.length);
  return max === 0 ? 0 : 1 - d / max;
}

export function scoreMatch(
  baseSlug: string,
  ship: ShipSlugIndex,
  folderLower: string
): { score: number; kind: string } {
  const aliasTarget = loadAliasMap()[baseSlug];
  if (aliasTarget && ship.displayName === aliasTarget) {
    return { score: 99, kind: "alias" };
  }

  const { fullSlug, shortSlug: short, isMeta } = ship;
  const wantsMeta = /meta/i.test(folderLower);

  if (baseSlug.length <= 2) {
    if (baseSlug === fullSlug || baseSlug === ship.displayName) {
      return { score: 100, kind: "exact_short_name" };
    }
    return { score: 0, kind: "none" };
  }

  if (baseSlug === fullSlug) {
    return { score: wantsMeta === isMeta ? 100 : isMeta ? 92 : 98, kind: "exact_full" };
  }
  if (baseSlug === short && short.length >= 3) {
    return { score: wantsMeta === isMeta ? 98 : isMeta ? 90 : 96, kind: "exact_short" };
  }
  if (fullSlug.length >= 5 && fullSlug.endsWith(baseSlug) && baseSlug.length >= 5) {
    return {
      score: Math.round(86 + (baseSlug.length / fullSlug.length) * 10) - (isMeta && !wantsMeta ? 8 : 0),
      kind: "suffix_ship",
    };
  }
  if (fullSlug.length >= 5 && fullSlug.includes(baseSlug) && baseSlug.length >= 5) {
    return {
      score: Math.round(82 + (baseSlug.length / fullSlug.length) * 10) - (isMeta && !wantsMeta ? 8 : 0),
      kind: "contains_ship",
    };
  }
  if (fullSlug.length >= 4 && fullSlug.startsWith(baseSlug) && baseSlug.length >= 4) {
    const ratio = baseSlug.length / fullSlug.length;
    return {
      score: Math.round(88 + ratio * 10) - (isMeta && !wantsMeta ? 8 : 0),
      kind: "prefix_ship",
    };
  }
  if (baseSlug.length >= 4 && baseSlug.startsWith(fullSlug)) {
    return { score: isMeta && !wantsMeta ? 80 : 87, kind: "prefix_folder" };
  }
  if (short.length >= 4 && (short.startsWith(baseSlug) || baseSlug.startsWith(short))) {
    return { score: isMeta && !wantsMeta ? 78 : 84, kind: "prefix_short" };
  }

  for (const candidate of [fullSlug, short]) {
    if (candidate.length < 4 || baseSlug.length < 4) continue;
    const ratio = fuzzyRatio(baseSlug, candidate);
    if (ratio >= 0.78) {
      const base = Math.round(72 + ratio * 25);
      return {
        score: base - (isMeta && !wantsMeta ? 10 : 0),
        kind: "fuzzy",
      };
    }
  }

  // 纯数字文件夹（如 22、33）精确匹配 display_name
  if (/^\d+$/.test(baseSlug) && toPinyinSlug(ship.displayName) === baseSlug) {
    return { score: 100, kind: "numeric_name" };
  }
  if (/^\d+$/.test(baseSlug) && ship.displayName === baseSlug) {
    return { score: 100, kind: "numeric_display" };
  }

  return { score: 0, kind: "none" };
}

export function buildShipSlugIndex(
  entries: Array<{ wikiTitle: string; displayName: string; aliasesJson?: string }>
): ShipSlugIndex[] {
  return entries.map((e) => {
    const fullSlug = toPinyinSlug(e.displayName);
    return {
      wikiTitle: e.wikiTitle,
      displayName: e.displayName,
      fullSlug,
      shortSlug: shortSlug(fullSlug),
      isMeta: /meta/i.test(e.displayName) || /·META/i.test(e.displayName),
    };
  });
}

export function scanLive2dFolders(root: string): Live2dFolder[] {
  if (!fs.existsSync(root)) {
    throw new Error(`Live2D 目录不存在: ${root}`);
  }
  const items: Live2dFolder[] = [];
  for (const ent of fs.readdirSync(root, { withFileTypes: true })) {
    if (!ent.isDirectory()) continue;
    const folder = ent.name;
    const folderPath = path.join(root, folder);
    const files = fs.readdirSync(folderPath);
    const skelFile = files.find((f) => f.toLowerCase().endsWith(".skel"));
    const { base, skinSuffix } = stripSkinSuffix(folder);
    const baseSlug = stripClassSuffix(base.toLowerCase());
    items.push({
      folder,
      folderPath,
      baseSlug,
      skinSuffix,
      skelFile: skelFile ?? "",
      hasSpine: Boolean(skelFile),
    });
  }
  return items.sort((a, b) => a.folder.localeCompare(b.folder));
}

function candidateShips(baseSlug: string, index: ShipSlugIndex[]): ShipSlugIndex[] {
  if (baseSlug.length <= 2) {
    return index.filter((s) => s.fullSlug === baseSlug || s.displayName === baseSlug);
  }
  const prefix = baseSlug.slice(0, 2);
  const out: ShipSlugIndex[] = [];
  for (const ship of index) {
    const { fullSlug, shortSlug: short } = ship;
    if (
      fullSlug.startsWith(prefix) ||
      short.startsWith(prefix) ||
      baseSlug.startsWith(fullSlug.slice(0, 3)) ||
      (fullSlug.length >= 5 && fullSlug.includes(baseSlug)) ||
      (baseSlug.length >= 5 && baseSlug.includes(fullSlug.slice(0, 4)))
    ) {
      out.push(ship);
    }
  }
  return out.length > 0 ? out : index;
}

export function matchLive2dFolders(
  folders: Live2dFolder[],
  index: ShipSlugIndex[],
  minScore = 70
): Live2dMatch[] {
  const aliasMap = loadAliasMap();
  const byDisplay = new Map(index.map((s) => [s.displayName, s]));
  const byFullSlug = new Map<string, ShipSlugIndex[]>();
  for (const ship of index) {
    const list = byFullSlug.get(ship.fullSlug) ?? [];
    list.push(ship);
    byFullSlug.set(ship.fullSlug, list);
  }

  const results: Live2dMatch[] = [];

  for (const f of folders) {
    if (!f.hasSpine) continue;

    const scored: Array<ShipSlugIndex & { score: number; matchKind: string }> = [];

    const aliasName = aliasMap[f.baseSlug];
    if (aliasName) {
      const ship = byDisplay.get(aliasName);
      if (ship) scored.push({ ...ship, score: 99, matchKind: "alias" });
    }

    const exact = byFullSlug.get(f.baseSlug);
    if (exact) {
      for (const ship of exact) {
        const { score, kind } = scoreMatch(f.baseSlug, ship, f.folder.toLowerCase());
        if (score >= minScore) scored.push({ ...ship, score, matchKind: kind });
      }
    }

    if (scored.length === 0) {
      for (const ship of candidateShips(f.baseSlug, index)) {
        const { score, kind } = scoreMatch(f.baseSlug, ship, f.folder.toLowerCase());
        if (score >= minScore) scored.push({ ...ship, score, matchKind: kind });
      }
    }

    scored.sort((a, b) => b.score - a.score || a.displayName.localeCompare(b.displayName));

    const best = scored[0];
    if (!best) {
      results.push({
        folder: f.folder,
        folderPath: f.folderPath,
        skinLabel: skinLabelFromSuffix(f.skinSuffix, f.folder),
        baseSlug: f.baseSlug,
        wikiTitle: "",
        displayName: "",
        score: 0,
        matchKind: "unmatched",
      });
      continue;
    }

    const ambiguous =
      scored.length > 1 && scored[1].score >= best.score - 5
        ? scored.slice(1, 4).map((s) => ({
            displayName: s.displayName,
            wikiTitle: s.wikiTitle,
            score: s.score,
          }))
        : undefined;

    results.push({
      folder: f.folder,
      folderPath: f.folderPath,
      skinLabel: skinLabelFromSuffix(f.skinSuffix, f.folder),
      baseSlug: f.baseSlug,
      wikiTitle: best.wikiTitle,
      displayName: best.displayName,
      score: best.score,
      matchKind: best.matchKind,
      ambiguous,
    });
  }

  return results;
}

/** 读取 HANDAILY 已导入 persona（character id = persona id） */
export function loadHandailyPersonaNames(dataDir: string): Map<string, string> {
  const manifestPath = path.join(dataDir, "personas", "manifest.json");
  if (!fs.existsSync(manifestPath)) return new Map();
  try {
    const raw = JSON.parse(fs.readFileSync(manifestPath, "utf8")) as {
      personas?: Array<{ id: string; name: string }>;
    };
    const map = new Map<string, string>();
    for (const p of raw.personas ?? []) {
      map.set(p.name, p.id);
      map.set(p.id, p.id);
    }
    return map;
  } catch {
    return new Map();
  }
}

export function loadHandailyModelNames(dataDir: string): Set<string> {
  const modelsDir = path.join(dataDir, "pet-models");
  const names = new Set<string>();
  if (!fs.existsSync(modelsDir)) return names;
  for (const ent of fs.readdirSync(modelsDir, { withFileTypes: true })) {
    if (!ent.isDirectory()) continue;
    const nameFile = path.join(modelsDir, ent.name, "name.txt");
    if (fs.existsSync(nameFile)) {
      names.add(fs.readFileSync(nameFile, "utf8").trim());
    }
  }
  return names;
}

export function buildImportPlan(
  matches: Live2dMatch[],
  opts: {
    handailyDataDir?: string;
    minScore?: number;
    onlyWithPersona?: boolean;
  } = {}
): Live2dImportPlanItem[] {
  const minScore = opts.minScore ?? 80;
  const personaMap = opts.handailyDataDir
    ? loadHandailyPersonaNames(opts.handailyDataDir)
    : new Map<string, string>();
  const existingModels = opts.handailyDataDir
    ? loadHandailyModelNames(opts.handailyDataDir)
    : new Set<string>();

  return matches.map((m) => {
    const skinName = m.skinLabel;
    const modelName =
      skinName === "默认" ? m.displayName : `${m.displayName}·${skinName}`;
    const characterId = personaMap.get(m.displayName);
    const personaImported = Boolean(characterId);
    const alreadyImported = existingModels.has(modelName);

    let action: Live2dImportPlanItem["action"] = "import";
    if (m.score < minScore) action = "skip_low_score";
    else if (opts.onlyWithPersona && !personaImported) action = "skip_no_persona";
    else if (alreadyImported) action = "skip_exists";

    return {
      folder: m.folder,
      folderPath: m.folderPath,
      skinName,
      modelName,
      wikiTitle: m.wikiTitle,
      displayName: m.displayName,
      score: m.score,
      characterId,
      personaImported,
      alreadyImported,
      action,
    };
  });
}

export function matchFromDatabase(
  db: BlhxDatabase,
  live2dRoot: string,
  minScore = 70
): { folders: Live2dFolder[]; matches: Live2dMatch[]; index: ShipSlugIndex[] } {
  const catalog = db.listCatalog({ fetched: "all", limit: 10000, offset: 0 });
  const index = buildShipSlugIndex(
    catalog.map((c) => ({
      wikiTitle: c.wikiTitle,
      displayName: c.displayName,
    }))
  );
  const folders = scanLive2dFolders(live2dRoot);
  const matches = matchLive2dFolders(folders, index, minScore);
  return { folders, matches, index };
}
