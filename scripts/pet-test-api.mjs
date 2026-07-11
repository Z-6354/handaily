#!/usr/bin/env node
/**
 * 桌宠 debug 测试 API 客户端（供 AI / CI 脚本调用）
 * 需先运行 `npm run tauri dev`，默认 http://127.0.0.1:19420
 *
 * 示例:
 *   node scripts/pet-test-api.mjs health
 *   node scripts/pet-test-api.mjs snapshot
 *   node scripts/pet-test-api.mjs switch next-skin
 *   node scripts/pet-test-api.mjs favorites
 *   node scripts/pet-test-api.mjs logs --n 20
 */

const BASE = process.env.HANDAILY_TEST_API_URL ?? "http://127.0.0.1:19420";
const DEFAULT_TIMEOUT_MS = Number(process.env.HANDAILY_TEST_API_TIMEOUT_MS ?? 60_000);

async function request(method, path, body, timeoutMs = DEFAULT_TIMEOUT_MS) {
  const res = await fetch(`${BASE}${path}`, {
    method,
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
    signal: AbortSignal.timeout(timeoutMs),
  });
  const text = await res.text();
  let data;
  try {
    data = JSON.parse(text);
  } catch {
    data = { raw: text };
  }
  if (!res.ok || (data && data.ok === false)) {
    const err = new Error(data?.error ?? `HTTP ${res.status}`);
    err.status = res.status;
    err.data = data;
    throw err;
  }
  return data;
}

function parseArgs(argv) {
  const out = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith("--")) {
      const key = a.slice(2);
      const next = argv[i + 1];
      if (next && !next.startsWith("--")) {
        out[key] = next;
        i++;
      } else {
        out[key] = true;
      }
    } else {
      out._.push(a);
    }
  }
  return out;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const [cmd, sub, ...rest] = args._;
  const timeoutMs = Number(args.timeout ?? args["timeout-ms"] ?? 30_000);

  let result;
  switch (cmd) {
    case undefined:
    case "help":
      result = await request("GET", "/");
      console.log(JSON.stringify(result, null, 2));
      return;
    case "health":
      result = await request("GET", "/health");
      break;
    case "snapshot":
      result = await request("GET", "/pet/snapshot");
      break;
    case "status":
      result = await request("GET", "/pet/status");
      break;
    case "skins":
      result = await request("GET", "/pet/menu/skins");
      break;
    case "characters":
      result = await request("GET", "/pet/characters");
      break;
    case "favorites":
      result = await request("GET", "/pet/characters/favorites");
      break;
    case "logs": {
      const n = args.n ?? 40;
      result = await request("GET", `/pet/logs/tail?n=${n}`);
      break;
    }
    case "menu":
      if (sub === "open") result = await request("POST", "/pet/menu/open");
      else if (sub === "hide") result = await request("POST", "/pet/menu/hide");
      else throw new Error(`unknown menu subcommand: ${sub}`);
      break;
    case "switch":
      if (sub === "next-skin") {
        result = await request("POST", "/pet/switch/next-skin", { timeout_ms: timeoutMs });
      } else if (sub === "next-character") {
        result = await request("POST", "/pet/switch/next-character", { timeout_ms: timeoutMs });
      } else if (sub === "skin") {
        const characterId = args.character ?? args.character_id;
        const skinId = args.skin ?? args.skin_id;
        if (!characterId || !skinId) {
          throw new Error("switch skin requires --character and --skin");
        }
        result = await request("POST", "/pet/switch/skin", {
          character_id: characterId,
          skin_id: skinId,
          timeout_ms: timeoutMs,
        });
      } else if (sub === "character") {
        const characterId = args.character ?? args.character_id ?? rest[0];
        if (!characterId) throw new Error("switch character requires --character <id>");
        result = await request("POST", "/pet/switch/character", {
          character_id: characterId,
          timeout_ms: timeoutMs,
        });
      } else {
        throw new Error(`unknown switch subcommand: ${sub}`);
      }
      break;
    case "exit":
      result = await request("POST", "/app/exit");
      break;
    default:
      throw new Error(`unknown command: ${cmd}`);
  }

  console.log(JSON.stringify(result, null, 2));
}

main().catch((err) => {
  console.error(err.message ?? err);
  if (err.data) console.error(JSON.stringify(err.data, null, 2));
  process.exit(1);
});
