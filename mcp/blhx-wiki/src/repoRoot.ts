import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

/** HANDAILY monorepo root (contains hanpet/, data/, package.json). */
export function repoRoot(): string {
  const env = process.env.HANDAILY_ROOT?.trim();
  if (env) return path.resolve(env);

  let dir = path.dirname(fileURLToPath(import.meta.url));
  for (let i = 0; i < 10; i++) {
    if (
      fs.existsSync(path.join(dir, "hanpet", "package.json")) &&
      fs.existsSync(path.join(dir, "data"))
    ) {
      return dir;
    }
    const parent = path.dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
}
