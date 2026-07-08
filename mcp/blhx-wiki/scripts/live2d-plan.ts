#!/usr/bin/env tsx
/**
 * 生成 Live2D 导入计划 JSON，供 live2d_import CLI 消费。
 *
 * 用法:
 *   npm run live2d-plan -- --out plan.json
 *   npm run live2d-plan -- --min-score 80 --only-with-persona
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { BlhxDatabase } from "../src/db.js";
import {
  buildImportPlan,
  defaultLive2dRoot,
  matchFromDatabase,
} from "../src/live2d.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function handailyDataDir(): string | undefined {
  const env = process.env.HANDAILY_DATA_DIR?.trim();
  if (env) return env;
  const appdata = process.env.APPDATA?.trim();
  if (appdata) return path.join(appdata, "xiaohan-daily", "data");
  return undefined;
}

function parseArgs(): {
  out?: string;
  live2dRoot?: string;
  minScore: number;
  onlyWithPersona: boolean;
} {
  const args = process.argv.slice(2);
  let out: string | undefined;
  let live2dRoot: string | undefined;
  let minScore = 80;
  let onlyWithPersona = true;

  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === "--out" && args[i + 1]) out = args[++i];
    else if (a === "--live2d-root" && args[i + 1]) live2dRoot = args[++i];
    else if (a === "--min-score" && args[i + 1]) minScore = Number(args[++i]);
    else if (a === "--all-personas") onlyWithPersona = false;
    else if (a === "--only-with-persona") onlyWithPersona = true;
  }

  return { out, live2dRoot, minScore, onlyWithPersona };
}

async function main(): Promise<void> {
  const { out, live2dRoot, minScore, onlyWithPersona } = parseArgs();
  const root = live2dRoot ? path.resolve(live2dRoot) : defaultLive2dRoot();
  const dataDir = handailyDataDir();

  const db = new BlhxDatabase();
  const { matches } = matchFromDatabase(db, root, 70);
  const plan = buildImportPlan(matches, {
    handailyDataDir: dataDir,
    minScore,
    onlyWithPersona,
  }).filter((p) => p.action === "import");

  const payload = {
    generatedAt: new Date().toISOString(),
    live2dRoot: root,
    handailyDataDir: dataDir ?? null,
    minScore,
    onlyWithPersona,
    count: plan.length,
    plan,
  };

  const json = JSON.stringify(payload, null, 2);
  if (out) {
    fs.writeFileSync(path.resolve(out), json, "utf8");
    console.log(`已写入 ${plan.length} 条导入计划 → ${path.resolve(out)}`);
  } else {
    console.log(json);
  }

  db.close();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
