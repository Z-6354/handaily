#!/usr/bin/env node
/**
 * CLI wrapper for iterative-hardening MCP store (no stdio transport).
 * Usage: node scripts/cli.mjs init | status | next | verify
 */
import { HardeningStore } from "../dist/store.js";

const cmd = process.argv[2];
const store = new HardeningStore(process.env.HANDAILY_ROOT ?? process.cwd());

function print(data) {
  console.log(JSON.stringify(data, null, 2));
}

if (cmd === "init") {
  const userReport = process.argv[3] ?? "桌宠编辑范围/模型移动/模型显示";
  const scenarios = [
    { id: "S1", steps: "编辑模式拖北边 resize", expected: "仅上边动，模型屏幕位置不变", status: "FAIL" },
    { id: "S2", steps: "编辑模式拖西边 resize", expected: "仅左边动，模型屏幕位置不变", status: "FAIL" },
    { id: "S3", steps: "编辑中拖角色偏移", expected: "松手后仍可再拖，不锁死", status: "UNTESTED" },
    { id: "S4", steps: "多次进入/退出编辑", expected: "offset 不累积漂移", status: "UNTESTED" },
    { id: "S5", steps: "编辑中 resize 后换皮肤(reload)", expected: "窗口尺寸不缩回 DB 旧值", status: "FAIL" },
    { id: "S6", steps: "全屏隐藏后恢复", expected: "非编辑态，无残留 poll", status: "UNTESTED" },
    { id: "S7", steps: "退出编辑后模型显示", expected: "与编辑中 offset 一致，无跳变", status: "FAIL" },
    { id: "S8", steps: "125%+ DPI 拖窗口保存", expected: "位置 clamp 正确，不半出屏", status: "UNTESTED" },
  ];
  print({ session: store.init(userReport, scenarios), firstPass: store.nextPass() });
} else if (cmd === "status") {
  print(store.getStatus());
} else if (cmd === "next") {
  print(store.nextPass());
} else if (cmd === "can-finish") {
  print(store.checkDone());
} else if (cmd === "submit-pass2") {
  const result = store.submitPass({
    reproEvidence:
      "failure-modes 审计：exitEditBounds/pet-hidden 静默 catch；隐藏时仍走完整 exit IPC；屏幕边缘 resize 锚点被 clamp 破坏；offset 松手不写 DB",
    findings:
      "exitEditBounds 双次 save_layout 且失败静默；pet-hidden 调 exitEditBounds 在窗口不可见时易失败；computeResizeBounds 未修正 clamp 后锚点；offset 拖完仅内存",
    fixes:
      "main.ts: persistLayoutSnapshotSafe+abandonEditBoundsOnHidden；exit 去重保存+console.error；computeResizeBounds 锚点回算；offset mouseup 持久化；mcp.json 加 cwd",
    verification: [
      { command: "cargo check -p xiaohan-daily --lib", exitCode: 0, summary: "pass2 verify" },
      { command: "npm run build -w hanpet", exitCode: 0, summary: "pass2 verify" },
    ],
    retest: "S6 代码路径改为 abandonEditBoundsOnHidden；S1/S2 锚点修复待 UI 验证；其余 UNTESTED",
    scenarioUpdates: {
      S1: "UNTESTED",
      S2: "UNTESTED",
      S3: "UNTESTED",
      S4: "UNTESTED",
      S5: "UNTESTED",
      S6: "UNTESTED",
      S7: "UNTESTED",
      S8: "UNTESTED",
    },
    next: "continue",
  });
  print(result);
} else {
  console.error("Usage: cli.mjs init|status|next|can-finish|submit-pass1|submit-pass2");
  process.exit(1);
}
