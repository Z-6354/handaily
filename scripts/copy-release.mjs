/**
 * 将 Tauri 构建产物复制到仓库根目录 release/，便于分发。
 */
import { copyFileSync, existsSync, mkdirSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const releaseDir = join(root, "release");
const targetRelease = join(root, "src-tauri", "target", "release");
const nsisDir = join(targetRelease, "bundle", "nsis");
const standaloneExe = join(targetRelease, "xiaohan-daily.exe");

mkdirSync(releaseDir, { recursive: true });

const copied = [];

if (existsSync(standaloneExe)) {
  const dest = join(releaseDir, "xiaohan-daily.exe");
  copyFileSync(standaloneExe, dest);
  copied.push(dest);
}

if (existsSync(nsisDir)) {
  for (const name of readdirSync(nsisDir)) {
    if (!name.endsWith(".exe")) continue;
    const dest = join(releaseDir, name);
    copyFileSync(join(nsisDir, name), dest);
    copied.push(dest);
  }
}

if (copied.length === 0) {
  console.error("[copy-release] 未找到构建产物，请先运行 npm run tauri:release");
  process.exit(1);
}

console.log("[copy-release] 已复制到 release/:");
for (const p of copied) {
  console.log(" ", p);
}
