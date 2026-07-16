#!/usr/bin/env node
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

const apiBase = (process.env.HANTRANSFER_URL ?? "http://127.0.0.1:7822").replace(/\/$/, "");

const server = new McpServer({
  name: "hantransfer",
  version: "1.0.0",
});

function jsonText(data: unknown): { content: Array<{ type: "text"; text: string }> } {
  return {
    content: [{ type: "text", text: JSON.stringify(data, null, 2) }],
  };
}

async function apiFetch(
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

server.tool(
  "snapshot",
  "获取 hantransfer 完整状态：已信任设备、待确认、待收件、推送队列。需 PC 已运行 npm run hantransfer。",
  {},
  async () => {
    const r = await apiFetch("GET", "/api/v1/agent/snapshot");
    return jsonText(r);
  },
);

server.tool(
  "list_devices",
  "列出已信任手机设备（从 snapshot 提取 trusted）。",
  {},
  async () => {
    const r = await apiFetch("GET", "/api/v1/agent/snapshot");
    if (!r.ok || typeof r.data !== "object" || r.data === null) {
      return jsonText(r);
    }
    const data = r.data as { data?: { trusted?: unknown } };
    return jsonText({ ok: true, trusted: data.data?.trusted ?? [] });
  },
);

server.tool(
  "push_files",
  "将本机文件推送到已信任手机。paths 为 PC 上的绝对路径。",
  {
    device_id: z.string().uuid().describe("目标手机 device_id（snapshot.trusted[].device_id）"),
    paths: z.array(z.string().min(1)).min(1).describe("本机绝对路径列表"),
  },
  async ({ device_id, paths }) => {
    const r = await apiFetch("POST", "/api/v1/agent/push", { device_id, paths });
    return jsonText(r);
  },
);

server.tool(
  "accept_receive",
  "接受手机发来的待收文件。不传 id 则全部接受。",
  {
    id: z.string().uuid().optional().describe("单个 transfer id；省略则 accept-all"),
  },
  async ({ id }) => {
    const r = await apiFetch("POST", "/api/v1/agent/receive/accept", id ? { id } : {});
    return jsonText(r);
  },
);

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
