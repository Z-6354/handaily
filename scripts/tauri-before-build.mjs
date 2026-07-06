/**
 * Tauri 打包前前端构建：自动检测是否仅需编 Rust，若是则跳过 vite/tsc。
 *
 * 环境变量（手动覆盖自动检测）：
 *   SKIP_FE_BUILD=1   强制跳过
 *   FORCE_FE_BUILD=1  强制构建
 *   FE_BUILD_FULL=1   使用 tsc + vite（发布前）
 */
import { execSync } from "node:child_process";
import { existsSync, readdirSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

/** 影响前端产物的输入路径（相对仓库根） */
const FE_INPUTS = [
  "src",
  "index.html",
  "pet.html",
  "vite.config.ts",
  "tsconfig.json",
  "package.json",
  "package-lock.json",
  "public",
];

const FE_INPUTS_FULL = ["tsconfig.app.json", "tsconfig.node.json"];

const DIST_MARKERS = ["dist/index.html", "dist/pet.html"];

const SKIP_DIR = new Set([
  "node_modules",
  "dist",
  "target",
  ".git",
  ".cursor",
]);

function walkNewestMtime(absDir, best = 0) {
  let max = best;
  for (const entry of readdirSync(absDir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      if (SKIP_DIR.has(entry.name)) continue;
      max = walkNewestMtime(join(absDir, entry.name), max);
      continue;
    }
    max = Math.max(max, statSync(join(absDir, entry.name)).mtimeMs);
  }
  return max;
}

function pathNewestMtime(absPath) {
  if (!existsSync(absPath)) return 0;
  const st = statSync(absPath);
  if (st.isDirectory()) return walkNewestMtime(absPath);
  return st.mtimeMs;
}

/** 前端输入文件的最大 mtime（ms） */
export function newestFrontendInputMtime(projectRoot, fullTypecheck = false) {
  const paths = fullTypecheck ? [...FE_INPUTS, ...FE_INPUTS_FULL] : FE_INPUTS;
  let max = 0;
  for (const rel of paths) {
    max = Math.max(max, pathNewestMtime(join(projectRoot, rel)));
  }
  return max;
}

/** dist 是否齐全 */
export function distArtifactsReady(projectRoot) {
  return DIST_MARKERS.every((rel) => existsSync(join(projectRoot, rel)));
}

/** dist 标记文件中最旧的 mtime：若仍新于全部前端输入，则产物有效 */
export function oldestDistMarkerMtime(projectRoot) {
  let min = Infinity;
  for (const rel of DIST_MARKERS) {
    const p = join(projectRoot, rel);
    if (!existsSync(p)) return 0;
    min = Math.min(min, statSync(p).mtimeMs);
  }
  return min === Infinity ? 0 : min;
}

/** dist 是否比所有前端输入都新（可跳过前端构建） */
export function isFrontendDistUpToDate(projectRoot, fullTypecheck = false) {
  if (!distArtifactsReady(projectRoot)) return false;
  const srcNewest = newestFrontendInputMtime(projectRoot, fullTypecheck);
  if (srcNewest === 0) return false;
  return oldestDistMarkerMtime(projectRoot) >= srcNewest;
}

function main() {
  if (process.env.SKIP_FE_BUILD === "1") {
    if (distArtifactsReady(root)) {
      console.log("[before-build] SKIP_FE_BUILD=1，跳过前端构建");
      process.exit(0);
    }
    console.warn("[before-build] SKIP_FE_BUILD=1 但 dist 不完整，仍执行构建");
  }

  if (process.env.FORCE_FE_BUILD === "1") {
    console.log("[before-build] FORCE_FE_BUILD=1，强制前端构建");
  } else {
    const full = process.env.FE_BUILD_FULL === "1";
    if (isFrontendDistUpToDate(root, full)) {
      console.log(
        "[before-build] 前端输入未变化（仅 Rust/后端改动），跳过 vite" +
          (full ? "/tsc" : "") +
          " 构建",
      );
      process.exit(0);
    }
  }

  const cmd = process.env.FE_BUILD_FULL === "1" ? "npm run build" : "npm run build:fe";
  console.log(`[before-build] 前端有变更或 dist 缺失，执行: ${cmd}`);
  execSync(cmd, { cwd: root, stdio: "inherit" });
}

main();
