import { parseCatalogEntries, parseShipPage } from "./scraper.js";
import type { CatalogEntry, ShipRecord } from "./types.js";

const WIKI_BASE = "https://wiki.biligame.com/blhx";
const USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 HANDAILY-blhx-wiki/1.0";

const DEFAULT_HEADERS = {
  "User-Agent": USER_AGENT,
  Accept: "application/json,text/html,*/*",
  "Accept-Language": "zh-CN,zh;q=0.9",
  Referer: `${WIKI_BASE}/`,
};

const CATALOG_PAGE = "舰船图鉴";
const REQUEST_DELAY_MS = Number(process.env.BLHX_WIKI_DELAY_MS ?? 350);
const MAX_RETRIES = 3;

let lastRequestAt = 0;

async function throttle(): Promise<void> {
  const now = Date.now();
  const wait = REQUEST_DELAY_MS - (now - lastRequestAt);
  if (wait > 0) await sleep(wait);
  lastRequestAt = Date.now();
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

export function wikiUrlForTitle(title: string): string {
  return `${WIKI_BASE}/${encodeURIComponent(title)}`;
}

async function fetchWithRetry(url: string): Promise<Response> {
  let lastError: Error | null = null;
  for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
    await throttle();
    try {
      const res = await fetch(url, { headers: DEFAULT_HEADERS });
      if (res.ok) return res;
      if (res.status === 567 || res.status === 429 || res.status >= 500) {
        lastError = new Error(`Wiki HTTP ${res.status}`);
        await sleep(800 * (attempt + 1));
        continue;
      }
      throw new Error(`Wiki HTTP ${res.status} for ${url}`);
    } catch (e) {
      lastError = e instanceof Error ? e : new Error(String(e));
      await sleep(800 * (attempt + 1));
    }
  }
  throw lastError ?? new Error(`Wiki fetch failed: ${url}`);
}

export async function fetchWikiHtml(pageTitle: string): Promise<string> {
  const apiUrl = `${WIKI_BASE}/api.php?action=parse&page=${encodeURIComponent(pageTitle)}&prop=text&format=json`;
  const res = await fetchWithRetry(apiUrl);
  const json = (await res.json()) as {
    error?: { info?: string };
    parse?: { text?: { "*": string } };
  };
  if (json.error?.info) throw new Error(json.error.info);
  const html = json.parse?.text?.["*"];
  if (html) return html;

  const pageRes = await fetchWithRetry(wikiUrlForTitle(pageTitle));
  const pageHtml = await pageRes.text();
  const start = pageHtml.indexOf('class="mw-parser-output"');
  if (start < 0) throw new Error(`Empty wiki page: ${pageTitle}`);
  const end = pageHtml.indexOf("</div>", start + 500);
  return pageHtml.slice(start, end > start ? end : start + 120_000);
}

export async function fetchCatalog(): Promise<CatalogEntry[]> {
  const html = await fetchWikiHtml(CATALOG_PAGE);
  return parseCatalogEntries(html);
}

export async function fetchShipRecord(
  wikiTitle: string,
  catalogMeta?: Partial<CatalogEntry>
): Promise<ShipRecord> {
  const html = await fetchWikiHtml(wikiTitle);
  const wikiUrl = wikiUrlForTitle(wikiTitle);
  const record = parseShipPage(html, wikiTitle, wikiUrl);
  if (catalogMeta) {
    record.rarity = catalogMeta.rarity ?? record.rarity;
    record.faction = catalogMeta.faction ?? record.faction;
    record.shipType = catalogMeta.shipType ?? record.shipType;
    if (catalogMeta.aliases?.length) {
      record.aliases = [...new Set([...record.aliases, ...catalogMeta.aliases])];
    }
  }
  return record;
}

export async function fetchShipRecordByName(name: string): Promise<ShipRecord> {
  return fetchShipRecord(name);
}
