#!/usr/bin/env node
import path from "node:path";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { BlhxDatabase } from "./db.js";
import { exportForHandaily, handailyImportGuide } from "./handaily.js";
import {
  buildImportPlan,
  defaultLive2dRoot,
  matchFromDatabase,
  scanLive2dFolders,
} from "./live2d.js";
import { fetchCatalog, fetchShipRecord } from "./wiki.js";

const db = new BlhxDatabase();

const server = new McpServer({
  name: "blhx-wiki",
  version: "1.0.0",
});

function jsonText(data: unknown): { content: Array<{ type: "text"; text: string }> } {
  return {
    content: [{ type: "text", text: JSON.stringify(data, null, 2) }],
  };
}

server.tool(
  "blhx_stats",
  "查看 BWIKI 舰娘数据库同步状态（图鉴总数、已抓取、待抓取）",
  {},
  async () => jsonText(db.stats())
);

server.tool(
  "blhx_sync_catalog",
  "从 BWIKI 舰船图鉴同步舰娘索引到本地数据库（增量 upsert）",
  {},
  async () => {
    const entries = await fetchCatalog();
    const result = db.upsertCatalog(entries);
    return jsonText({
      ok: true,
      catalogCount: entries.length,
      ...result,
      stats: db.stats(),
    });
  }
);

server.tool(
  "blhx_list_ships",
  "列出图鉴舰娘，可按是否已抓取过滤",
  {
    fetched: z
      .enum(["all", "yes", "no"])
      .optional()
      .describe("all=全部，yes=已抓取详情，no=待抓取"),
    limit: z.number().int().min(1).max(200).optional(),
    offset: z.number().int().min(0).optional(),
  },
  async ({ fetched = "all", limit = 50, offset = 0 }) =>
    jsonText({
      items: db.listCatalog({ fetched, limit, offset }),
      stats: db.stats(),
    })
);

server.tool(
  "blhx_search_ships",
  "按名称/别名搜索已抓取的舰娘详情",
  {
    query: z.string().min(1).describe("舰娘名称或别名关键词"),
    limit: z.number().int().min(1).max(50).optional(),
  },
  async ({ query, limit = 20 }) => {
    const items = db.searchShips(query, limit);
    return jsonText({ query, count: items.length, items });
  }
);

server.tool(
  "blhx_get_ship",
  "从本地数据库读取舰娘完整资料（台词、设定、图片等）",
  {
    name: z.string().min(1).describe("Wiki 标题或显示名，如 欧根亲王"),
  },
  async ({ name }) => {
    const ship = db.findShipByName(name);
    if (!ship) {
      return jsonText({
        ok: false,
        error: `本地未找到「${name}」；请先 blhx_sync_catalog + blhx_sync_ships 或 blhx_fetch_ship`,
        pending: !db.hasShip(name) && !!db.getCatalogEntry(name),
      });
    }
    return jsonText({ ok: true, ship });
  }
);

server.tool(
  "blhx_fetch_ship",
  "抓取单个舰娘 Wiki 页并写入数据库（已存在则跳过，除非 force=true）",
  {
    name: z.string().min(1).describe("Wiki 标题，如 欧根亲王"),
    force: z.boolean().optional().describe("强制重新抓取"),
  },
  async ({ name, force = false }) => {
    if (!force && db.hasShip(name)) {
      return jsonText({
        ok: true,
        skipped: true,
        reason: "already_exists",
        ship: db.getShip(name) ?? db.findShipByName(name),
      });
    }
    const meta = db.getCatalogEntry(name);
    const record = await fetchShipRecord(name, meta ?? undefined);
    db.saveShip(record);
    return jsonText({ ok: true, skipped: false, ship: record });
  }
);

server.tool(
  "blhx_sync_ships",
  "增量批量抓取舰娘详情：跳过本地已有记录，仅抓取缺失项",
  {
    limit: z
      .number()
      .int()
      .min(1)
      .max(50)
      .optional()
      .describe("本次最多抓取数量，默认 10"),
    force: z.boolean().optional().describe("忽略本地缓存，全部重新抓取"),
  },
  async ({ limit = 10, force = false }) => {
    const statsBefore = db.stats();
    if (statsBefore.catalogTotal === 0) {
      const entries = await fetchCatalog();
      db.upsertCatalog(entries);
    }

    const targets = force
      ? db.listCatalog({ fetched: "all", limit, offset: 0 }).map((c) => c.wikiTitle)
      : db.listPendingTitles(limit).map((c) => c.wikiTitle);

    const results: Array<{ name: string; ok: boolean; skipped?: boolean; error?: string }> = [];

    for (const name of targets) {
      try {
        if (!force && db.hasShip(name)) {
          results.push({ name, ok: true, skipped: true });
          continue;
        }
        const meta = db.getCatalogEntry(name);
        const record = await fetchShipRecord(name, meta ?? undefined);
        db.saveShip(record);
        results.push({ name, ok: true, skipped: false });
      } catch (e) {
        results.push({ name, ok: false, error: e instanceof Error ? e.message : String(e) });
      }
    }

    return jsonText({
      ok: true,
      processed: results.length,
      results,
      stats: db.stats(),
    });
  }
);

server.tool(
  "blhx_export_handaily",
  "导出适合 hanpet 人物/性格导入的结构化资料（persona 参考文本 + 台词 + 资源 URL）",
  {
    name: z.string().min(1).describe("舰娘名称"),
    include_guide: z.boolean().optional().describe("附加开发者导入说明 Markdown"),
  },
  async ({ name, include_guide = true }) => {
    const ship = db.findShipByName(name);
    if (!ship) {
      return jsonText({
        ok: false,
        error: `本地未找到「${name}」，请先抓取`,
      });
    }
    const exported = exportForHandaily(ship);
    const payload = include_guide
      ? { ok: true, export: exported, guide: handailyImportGuide(exported) }
      : { ok: true, export: exported };
    return jsonText(payload);
  }
);

function handailyDataDir(): string | undefined {
  const env = process.env.HANDAILY_DATA_DIR?.trim();
  if (env) return env;
  const appdata = process.env.APPDATA?.trim();
  if (appdata) return `${appdata}\\xiaohan-daily\\data`;
  return undefined;
}

server.tool(
  "blhx_scan_live2d",
  "扫描 live2d 目录，列出含 Spine 三件套（.skel/.atlas/.png）的模型文件夹",
  {
    live2d_root: z
      .string()
      .optional()
      .describe("Live2D 根目录，默认 HANDAILY_LIVE2D_PATH 或仓库 data/live2d/"),
    only_spine: z.boolean().optional().describe("仅返回含 .skel 的文件夹，默认 true"),
  },
  async ({ live2d_root, only_spine = true }) => {
    const root = live2d_root ? path.resolve(live2d_root) : defaultLive2dRoot();
    const folders = scanLive2dFolders(root);
    const items = only_spine ? folders.filter((f) => f.hasSpine) : folders;
    return jsonText({
      ok: true,
      live2dRoot: root,
      total: items.length,
      items,
    });
  }
);

server.tool(
  "blhx_match_live2d",
  "将 live2d 文件夹名（拼音 slug）与 BWIKI 图鉴舰娘匹配，支持 adaerbote / adaerbote_2 等变体",
  {
    live2d_root: z.string().optional(),
    min_score: z.number().min(0).max(100).optional().describe("最低匹配分，默认 70"),
    limit: z.number().int().min(1).max(2000).optional(),
    unmatched_only: z.boolean().optional(),
  },
  async ({ live2d_root, min_score = 70, limit, unmatched_only = false }) => {
    const root = live2d_root ? path.resolve(live2d_root) : defaultLive2dRoot();
    const { folders, matches } = matchFromDatabase(db, root, min_score);
    let items = matches;
    if (unmatched_only) items = items.filter((m) => !m.wikiTitle);
    if (limit) items = items.slice(0, limit);

    const matched = matches.filter((m) => m.wikiTitle).length;
    const unmatched = matches.filter((m) => !m.wikiTitle).length;
    const lowConfidence = matches.filter(
      (m) => m.wikiTitle && m.score < 85
    ).length;

    for (const m of matches.filter((x) => x.wikiTitle)) {
      db.saveLive2dMapping({
        folder: m.folder,
        wikiTitle: m.wikiTitle,
        displayName: m.displayName,
        skinLabel: m.skinLabel,
        score: m.score,
      });
    }

    return jsonText({
      ok: true,
      live2dRoot: root,
      catalogTotal: db.stats().catalogTotal,
      spineFolders: folders.filter((f) => f.hasSpine).length,
      matched,
      unmatched,
      lowConfidence,
      minScore: min_score,
      items,
    });
  }
);

server.tool(
  "blhx_live2d_import_plan",
  "生成 Live2D 模型导入计划：匹配舰娘 → 检查 HANDAILY 是否已导入人设/模型 → 输出待导入列表",
  {
    live2d_root: z.string().optional(),
    handaily_data_dir: z.string().optional().describe("hanpet 运行时 data 目录，默认 %AppData%/xiaohan-daily/data"),
    min_score: z.number().min(0).max(100).optional().describe("导入最低分，默认 80"),
    only_with_persona: z
      .boolean()
      .optional()
      .describe("仅包含已在 HANDAILY 导入人设的舰娘，默认 true"),
    action: z
      .enum(["all", "import", "skip"])
      .optional()
      .describe("过滤 action 类型，默认 import"),
  },
  async ({
    live2d_root,
    handaily_data_dir,
    min_score = 80,
    only_with_persona = true,
    action = "import",
  }) => {
    const root = live2d_root ? path.resolve(live2d_root) : defaultLive2dRoot();
    const dataDir = handaily_data_dir ?? handailyDataDir();
    const { matches } = matchFromDatabase(db, root, 70);
    const plan = buildImportPlan(matches, {
      handailyDataDir: dataDir,
      minScore: min_score,
      onlyWithPersona: only_with_persona,
    });

    const filtered =
      action === "all" ? plan : plan.filter((p) => p.action === action);
    const summary = {
      total: plan.length,
      toImport: plan.filter((p) => p.action === "import").length,
      skipLowScore: plan.filter((p) => p.action === "skip_low_score").length,
      skipNoPersona: plan.filter((p) => p.action === "skip_no_persona").length,
      skipExists: plan.filter((p) => p.action === "skip_exists").length,
    };

    return jsonText({
      ok: true,
      live2dRoot: root,
      handailyDataDir: dataDir ?? null,
      minScore: min_score,
      onlyWithPersona: only_with_persona,
      summary,
      plan: filtered,
      cliHint:
        "将 plan 写入 JSON 后执行: npm run live2d:import -- --plan data/import/live2d-plan.json（或 hanimport models）",
    });
  }
);

async function main(): Promise<void> {
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});

process.on("SIGINT", () => {
  db.close();
  process.exit(0);
});
