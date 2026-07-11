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
 *   node scripts/pet-test-api.mjs speak --text "你好"
 *   node scripts/pet-test-api.mjs speak random
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
      if (sub === "movement") {
        result = await request("GET", `/pet/logs/movement/tail?n=${n}`);
      } else {
        result = await request("GET", `/pet/logs/tail?n=${n}`);
      }
      break;
    }
    case "interaction":
      if (sub === "sync") result = await request("POST", "/pet/interaction/sync");
      else result = await request("GET", "/pet/interaction");
      break;
    case "bubble": {
      const enabled = sub === "on" || sub === "enable" || sub === "true";
      if (sub === "off" || sub === "disable" || sub === "false") {
        result = await request("POST", "/pet/bubble/set", { enabled: false });
      } else if (sub === "on" || sub === "enable" || sub === "true") {
        result = await request("POST", "/pet/bubble/set", { enabled: true });
      } else if (args.enabled !== undefined) {
        result = await request("POST", "/pet/bubble/set", {
          enabled: args.enabled === "true" || args.enabled === true,
        });
      } else {
        throw new Error("bubble requires on|off or --enabled true|false");
      }
      break;
    }
    case "main":
      if (sub === "open") result = await request("POST", "/pet/main/open");
      else if (sub === "close") result = await request("POST", "/pet/main/close");
      else throw new Error(`unknown main subcommand: ${sub}`);
      break;
    case "click":
      if (sub === "left") result = await request("POST", "/pet/click/left");
      else if (sub === "right") result = await request("POST", "/pet/click/right");
      else if (sub === "double") result = await request("POST", "/pet/click/double");
      else throw new Error(`unknown click subcommand: ${sub}`);
      break;
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
    case "speak":
      if (sub === "random") {
        result = await request("POST", "/pet/speak/random");
      } else {
        const text = args.text ?? rest.join(" ");
        if (!text) throw new Error("speak requires --text");
        result = await request("POST", "/pet/speak", {
          text,
          animation: args.animation ?? null,
        });
      }
      break;
    case "edit":
      if (sub === "enter") result = await request("POST", "/pet/edit/enter");
      else throw new Error(`unknown edit subcommand: ${sub}`);
      break;
    case "exit":
      result = await request("POST", "/app/exit");
      break;
    case "cursor":
      if (sub === "set") {
        const x = Number(args.x);
        const y = Number(args.y);
        if (!Number.isFinite(x) || !Number.isFinite(y)) {
          throw new Error("cursor set requires --x and --y");
        }
        result = await request("POST", "/system/cursor", { x, y });
      } else {
        result = await request("GET", "/system/cursor");
      }
      break;
    case "mouse": {
      const x = Number(args.x);
      const y = Number(args.y);
      if (!Number.isFinite(x) || !Number.isFinite(y)) {
        throw new Error("mouse requires --x and --y");
      }
      result = await request("POST", "/system/mouse", {
        x,
        y,
        button: args.button ?? sub ?? "left",
        action: args.action ?? "click",
      });
      break;
    }
    case "screenshot":
      if (sub === "pet") result = await request("GET", "/system/screenshot/pet");
      else {
        const maxW = args.max_width ?? args["max-width"] ?? 1280;
        result = await request("GET", `/system/screenshot?max_width=${maxW}`);
      }
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
