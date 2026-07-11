#!/usr/bin/env node
import { execSync } from "node:child_process";
import path from "node:path";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { HardeningStore } from "./store.js";
import type { ScenarioStatus } from "./types.js";

const workspaceRoot = process.env.HANDAILY_ROOT ?? process.cwd();
const store = new HardeningStore(workspaceRoot);

const server = new McpServer({
  name: "iterative-hardening",
  version: "1.0.0",
});

function jsonText(data: unknown): { content: Array<{ type: "text"; text: string }> } {
  return {
    content: [{ type: "text", text: JSON.stringify(data, null, 2) }],
  };
}

const scenarioStatusSchema = z.enum(["UNTESTED", "FAIL", "PASS", "BLOCKED"]);

const scenarioInputSchema = z.object({
  id: z.string().min(1),
  steps: z.string().min(1),
  expected: z.string().min(1),
  status: scenarioStatusSchema.optional(),
  notes: z.string().optional(),
});

server.tool(
  "hardening_init",
  "Phase 0: 从用户报告初始化 Scenario Matrix，开始循环修复会话",
  {
    user_report: z.string().min(1).describe("原始用户问题/bug 描述"),
    scenarios: z
      .array(scenarioInputSchema)
      .min(1)
      .describe("场景矩阵行，至少包含 S1（报告的主 bug）"),
  },
  async ({ user_report, scenarios }) => {
    const session = store.init(
      user_report,
      scenarios.map((s) => ({
        id: s.id,
        steps: s.steps,
        expected: s.expected,
        status: s.status ?? "UNTESTED",
        notes: s.notes,
      })),
    );
    const guide = store.nextPass();
    return jsonText({
      ok: true,
      message: "Session initialized. Call hardening_next_pass then follow the loop.",
      session: {
        sessionId: session.sessionId,
        status: session.status,
        scenarioCount: session.scenarioMatrix.length,
        matrix: session.scenarioMatrix,
      },
      firstPass: guide,
      protocol: "Reproduce → Audit → Fix → Verify → Retest ALL scenarios → hardening_submit_pass",
    });
  },
);

server.tool(
  "hardening_status",
  "获取当前循环修复会话状态与场景矩阵",
  {},
  async () => {
    const { session, message } = store.getStatus();
    const doneCheck = store.checkDone(session ?? undefined);
    return jsonText({ message, session, doneCheck });
  },
);

server.tool(
  "hardening_next_pass",
  "开始下一循环轮次：返回 pass 编号、审计角度与执行步骤清单",
  {},
  async () => {
    const guide = store.nextPass();
    const { session } = store.getStatus();
    return jsonText({ guide, session });
  },
);

server.tool(
  "hardening_submit_pass",
  "提交本轮 Loop pass 报告（必填：复现证据、发现、修复、验证、全矩阵重测结果）",
  {
    repro_evidence: z.string().min(1),
    findings: z.string().min(1).describe('审计发现；无则写 "none"'),
    fixes: z.string().min(1).describe("修改的文件与一行摘要"),
    verification: z
      .array(
        z.object({
          command: z.string(),
          exitCode: z.number().int().nullable(),
          summary: z.string(),
        }),
      )
      .min(1)
      .describe("本轮运行的验证命令及结果"),
    retest: z.string().min(1).describe("重测了整个矩阵的哪些行、结果如何"),
    scenario_updates: z
      .record(scenarioStatusSchema)
      .describe("各场景 ID 的最新状态，如 { S1: PASS, S2: FAIL }"),
    next: z.enum(["continue", "done"]).describe("认为可结束时传 done，须满足 exit criteria"),
  },
  async (input) => {
    const result = store.submitPass({
      reproEvidence: input.repro_evidence,
      findings: input.findings,
      fixes: input.fixes,
      verification: input.verification,
      retest: input.retest,
      scenarioUpdates: input.scenario_updates as Record<string, ScenarioStatus>,
      next: input.next,
    });
    const doneCheck = store.checkDone(result.session);
    return jsonText({
      ok: true,
      passReport: result.passReport,
      warnings: result.warnings,
      session: {
        status: result.session.status,
        passNumber: result.session.passNumber,
        matrix: result.session.scenarioMatrix,
      },
      doneCheck,
    });
  },
);

server.tool(
  "hardening_add_scenario",
  "循环中途新增回归场景行（不可删除已有行）",
  {
    id: z.string().min(1),
    steps: z.string().min(1),
    expected: z.string().min(1),
    status: scenarioStatusSchema.optional(),
    notes: z.string().optional(),
  },
  async (row) => {
    const session = store.addScenario(row);
    return jsonText({ ok: true, added: row.id, matrix: session.scenarioMatrix });
  },
);

server.tool(
  "hardening_update_scenario",
  "更新单个场景状态",
  {
    id: z.string().min(1),
    status: scenarioStatusSchema,
    notes: z.string().optional(),
  },
  async ({ id, status, notes }) => {
    const session = store.updateScenario(id, status, notes);
    return jsonText({ ok: true, id, status, matrix: session.scenarioMatrix });
  },
);

server.tool(
  "hardening_can_finish",
  "检查是否满足循环退出条件（全部 PASS + 审计干净 + 有验证证据）",
  {},
  async () => jsonText(store.checkDone()),
);

server.tool(
  "hardening_run_verify",
  "运行项目验证命令并返回 exit code 与输出摘要（供 submit_pass 引用）",
  {
    profile: z
      .enum(["rust", "frontend", "all", "custom"])
      .optional()
      .describe("预设：rust=cargo check, frontend=tsc, all=两者, custom=用 command"),
    command: z.string().optional().describe("profile=custom 时必填"),
    cwd: z.string().optional().describe("工作目录，默认 HANDAILY 根目录"),
  },
  async ({ profile = "all", command, cwd }) => {
    const root = cwd ?? workspaceRoot;
    const commands: string[] = [];
    if (profile === "rust") commands.push("cargo check --manifest-path src-tauri/Cargo.toml");
    if (profile === "frontend") commands.push("npx tsc --noEmit");
    if (profile === "all") {
      commands.push("cargo check --manifest-path src-tauri/Cargo.toml");
      commands.push("npx tsc --noEmit");
    }
    if (profile === "custom") {
      if (!command) throw new Error("command required when profile=custom");
      commands.push(command);
    }

    const results = commands.map((cmd) => {
      try {
        const out = execSync(cmd, {
          cwd: root,
          encoding: "utf8",
          stdio: ["ignore", "pipe", "pipe"],
          timeout: 300_000,
          windowsHide: true,
        });
        const summary = out.slice(-800) || "(no output)";
        return { command: cmd, exitCode: 0, summary };
      } catch (err: unknown) {
        const e = err as { status?: number; stdout?: string; stderr?: string };
        const combined = [e.stdout, e.stderr].filter(Boolean).join("\n").slice(-800);
        return {
          command: cmd,
          exitCode: e.status ?? 1,
          summary: combined || String(err),
        };
      }
    });

    return jsonText({
      ok: results.every((r) => r.exitCode === 0),
      results,
      hint: "Copy results into hardening_submit_pass verification field",
    });
  },
);

server.tool(
  "hardening_protocol",
  "返回完整循环修复协议（原 skill 内容摘要）",
  {},
  async () =>
    jsonText({
      protocol: store.getProtocolMarkdown(),
      auditAngles: [
        "correctness",
        "failure-modes",
        "concurrency",
        "boundaries",
        "regression",
        "resources",
      ],
      maxPasses: 12,
    }),
);

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
