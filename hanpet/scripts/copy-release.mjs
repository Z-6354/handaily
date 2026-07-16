/**
 * 将 Tauri NSIS 安装包复制到仓库根目录 release/（仅安装包，不含便携 exe）。
 */
import { copyFileSync, existsSync, mkdirSync, readdirSync, unlinkSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const hanpetRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const projectRoot = join(hanpetRoot, "..");
const releaseDir = join(projectRoot, "release");
const nsisDir = join(hanpetRoot, "src-tauri", "target", "release", "bundle", "nsis");

mkdirSync(releaseDir, { recursive: true });

// 清理 release/ 中旧的安装包，避免堆积
for (const name of readdirSync(releaseDir)) {
  if (!name.endsWith(".exe")) continue;
  unlinkSync(join(releaseDir, name));
}

const copied = [];

if (existsSync(nsisDir)) {
  for (const name of readdirSync(nsisDir)) {
    if (!name.endsWith(".exe")) continue;
    const dest = join(releaseDir, name);
    copyFileSync(join(nsisDir, name), dest);
    copied.push(dest);
  }
}

if (copied.length === 0) {
  console.error("[copy-release] 未找到 NSIS 安装包，请先运行 npm run tauri:build");
  process.exit(1);
}

console.log("[copy-release] 已复制安装包到 release/:");
for (const p of copied) {
  console.log(" ", p);
}
