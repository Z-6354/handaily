/**
 * 桌宠内置模型：dev 从 bundled/roster/pet-models 提供 /assets/pet，build 写入 dist/assets/pet。
 * 不再维护 public/assets/pet 副本。
 */
import { cpSync, createReadStream, existsSync, mkdirSync, readdirSync, statSync } from "node:fs";
import { dirname, extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";

const MIME: Record<string, string> = {
  ".json": "application/json",
  ".png": "image/png",
  ".atlas": "text/plain",
  ".skel": "application/octet-stream",
  ".md": "text/plain",
};

function projectRoot(): string {
  return join(dirname(fileURLToPath(import.meta.url)), "..");
}

function petModelsRoot(): string {
  return join(projectRoot(), "bundled/roster/pet-models");
}

function copyDir(src: string, dest: string): void {
  mkdirSync(dest, { recursive: true });
  for (const name of readdirSync(src)) {
    const from = join(src, name);
    const to = join(dest, name);
    if (statSync(from).isDirectory()) {
      copyDir(from, to);
    } else {
      cpSync(from, to);
    }
  }
}

function contentType(filePath: string): string {
  return MIME[extname(filePath).toLowerCase()] ?? "application/octet-stream";
}

export default function bundledPetPlugin(): Plugin {
  const root = petModelsRoot();
  return {
    name: "bundled-pet-assets",
    configureServer(server) {
      server.middlewares.use("/assets/pet", (req, res, next) => {
        if (!req.url) return next();
        const rel = decodeURIComponent(req.url.split("?")[0]);
        const file = normalize(join(root, rel.replace(/^\/+/, "")));
        if (!file.startsWith(normalize(root)) || !existsSync(file) || statSync(file).isDirectory()) {
          return next();
        }
        res.setHeader("Content-Type", contentType(file));
        createReadStream(file).pipe(res);
      });
    },
    closeBundle() {
      if (!existsSync(root)) return;
      const out = join(projectRoot(), "dist/assets/pet");
      copyDir(root, out);
    },
  };
}
