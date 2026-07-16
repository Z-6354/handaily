#!/usr/bin/env node
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

const apiBase = process.env.HANDAILY_TEST_API_URL ?? "http://127.0.0.1:19420";

const server = new McpServer({
  name: "handaily-pet",
  version: "1.0.0",
});

function jsonText(data: unknown): { content: Array<{ type: "text"; text: string }> } {
  return {
    content: [{ type: "text", text: JSON.stringify(data, null, 2) }],
  };
}

async function petFetch(
  method: string,
  path: string,
  body?: Record<string, unknown>,
): Promise<{ ok: boolean; status: number; data: unknown }> {
  const res = await fetch(`${apiBase}${path}`, {
    method,
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  let data: unknown = text;
  try {
    data = JSON.parse(text);
  } catch {
    /* keep raw */
  }
  const ok =
    res.ok &&
    typeof data === "object" &&
    data !== null &&
    (data as { ok?: boolean }).ok !== false;
  return { ok, status: res.status, data };
}

const actionSchema = z.enum([
  "health",
  "index",
  "snapshot",
  "status",
  "skins",
  "characters",
  "favorites",
  "logs",
  "speak",
  "speak_random",
  "preview_animation",
  "switch_next_skin",
  "switch_next_character",
  "switch_skin",
  "switch_character",
  "menu_open",
  "menu_hide",
  "edit_enter",
  "cursor_get",
  "cursor_set",
  "mouse_click",
  "screenshot",
  "screenshot_pet",
  "main_open",
  "main_close",
  "bubble_set",
  "interaction",
]);

server.tool(
  "pet_control",
  "控制小寒桌宠：说台词、切换模型、读状态等。需应用在设置中开启 Agent 控制接口。",
  {
    action: actionSchema.describe("控制动作"),
    text: z.string().optional().describe("speak 时的台词"),
    animation: z.string().optional().describe("speak / preview_animation 关联动画"),
    loop: z.boolean().optional().describe("preview_animation 是否循环"),
    character_id: z.string().optional(),
    skin_id: z.string().optional(),
    timeout_ms: z.number().int().positive().optional().describe("切换超时，默认 30000"),
    log_lines: z.number().int().positive().optional().describe("logs 行数，默认 40"),
    x: z.number().int().optional().describe("cursor_set / mouse_click 屏幕 X"),
    y: z.number().int().optional().describe("cursor_set / mouse_click 屏幕 Y"),
    button: z.enum(["left", "right"]).optional().describe("mouse_click 按键"),
    mouse_action: z.enum(["click", "down", "up"]).optional().describe("mouse_click 动作"),
    max_width: z.number().int().positive().optional().describe("screenshot 最大宽度"),
    enabled: z.boolean().optional().describe("bubble_set 开关"),
  },
  async ({
    action,
    text,
    animation,
    loop,
    character_id,
    skin_id,
    timeout_ms,
    log_lines,
    x,
    y,
    button,
    mouse_action,
    max_width,
    enabled,
  }) => {
    const timeout = timeout_ms ?? 30_000;
    try {
      let result: { ok: boolean; status: number; data: unknown };
      switch (action) {
        case "health":
          result = await petFetch("GET", "/health");
          break;
        case "index":
          result = await petFetch("GET", "/");
          break;
        case "snapshot":
          result = await petFetch("GET", "/pet/snapshot");
          break;
        case "status":
          result = await petFetch("GET", "/pet/status");
          break;
        case "skins":
          result = await petFetch("GET", "/pet/menu/skins");
          break;
        case "characters":
          result = await petFetch("GET", "/pet/characters");
          break;
        case "favorites":
          result = await petFetch("GET", "/pet/characters/favorites");
          break;
        case "logs": {
          const n = log_lines ?? 40;
          result = await petFetch("GET", `/pet/logs/tail?n=${n}`);
          break;
        }
        case "speak": {
          if (!text?.trim()) throw new Error("speak requires text");
          result = await petFetch("POST", "/pet/speak", { text, animation: animation ?? null });
          break;
        }
        case "speak_random":
          result = await petFetch("POST", "/pet/speak/random");
          break;
        case "preview_animation": {
          if (!animation?.trim()) throw new Error("preview_animation requires animation");
          result = await petFetch("POST", "/pet/preview/animation", {
            animation,
            loop: loop ?? false,
          });
          break;
        }
        case "switch_next_skin":
          result = await petFetch("POST", "/pet/switch/next-skin", { timeout_ms: timeout });
          break;
        case "switch_next_character":
          result = await petFetch("POST", "/pet/switch/next-character", { timeout_ms: timeout });
          break;
        case "switch_skin": {
          if (!character_id || !skin_id) {
            throw new Error("switch_skin requires character_id and skin_id");
          }
          result = await petFetch("POST", "/pet/switch/skin", {
            character_id,
            skin_id,
            timeout_ms: timeout,
          });
          break;
        }
        case "switch_character": {
          if (!character_id) throw new Error("switch_character requires character_id");
          result = await petFetch("POST", "/pet/switch/character", {
            character_id,
            timeout_ms: timeout,
          });
          break;
        }
        case "menu_open":
          result = await petFetch("POST", "/pet/menu/open");
          break;
        case "menu_hide":
          result = await petFetch("POST", "/pet/menu/hide");
          break;
        case "edit_enter":
          result = await petFetch("POST", "/pet/edit/enter");
          break;
        case "cursor_get":
          result = await petFetch("GET", "/system/cursor");
          break;
        case "cursor_set": {
          if (x === undefined || y === undefined) throw new Error("cursor_set requires x and y");
          result = await petFetch("POST", "/system/cursor", { x, y });
          break;
        }
        case "mouse_click": {
          if (x === undefined || y === undefined) throw new Error("mouse_click requires x and y");
          result = await petFetch("POST", "/system/mouse", {
            x,
            y,
            button: button ?? "left",
            action: mouse_action ?? "click",
          });
          break;
        }
        case "screenshot": {
          const mw = max_width ?? 1280;
          result = await petFetch("GET", `/system/screenshot?max_width=${mw}`);
          break;
        }
        case "screenshot_pet":
          result = await petFetch("GET", "/system/screenshot/pet");
          break;
        case "main_open":
          result = await petFetch("POST", "/pet/main/open");
          break;
        case "main_close":
          result = await petFetch("POST", "/pet/main/close");
          break;
        case "bubble_set": {
          if (typeof enabled !== "boolean") throw new Error("bubble_set requires enabled: boolean");
          result = await petFetch("POST", "/pet/bubble/set", { enabled });
          break;
        }
        case "interaction":
          result = await petFetch("GET", "/pet/interaction");
          break;
        default:
          throw new Error(`unknown action: ${action as string}`);
      }
      return jsonText({
        ...result,
        apiBase,
        hint:
          "Enable MCP in app Settings → Agent 控制接口, then restart. Override URL with HANDAILY_TEST_API_URL.",
        cli: `node scripts/pet-test-api.mjs ${action.replace(/_/g, " ")}`,
      });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      return jsonText({
        ok: false,
        error: message,
        apiBase,
        hint: "Is the app running with Agent control enabled?",
      });
    }
  },
);

const transport = new StdioServerTransport();
await server.connect(transport);
