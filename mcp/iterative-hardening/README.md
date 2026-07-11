# Iterative Hardening MCP (循环修复)

将 `iterative-code-hardening` skill 工作流封装为 MCP 工具，通过持久化 Scenario Matrix 与 Pass Report 强制完整循环。

## 工具

| 工具 | 用途 |
|------|------|
| `hardening_init` | Phase 0：初始化场景矩阵 |
| `hardening_status` | 查看当前会话 |
| `hardening_next_pass` | 获取本轮 pass 与审计角度 |
| `hardening_submit_pass` | 提交 pass 报告 |
| `hardening_add_scenario` | 中途新增回归场景 |
| `hardening_update_scenario` | 更新单场景状态 |
| `hardening_can_finish` | 检查退出条件 |
| `hardening_run_verify` | 运行 cargo check / tsc |
| `hardening_pet_api` | 桌宠 debug HTTP 测试 API（AI 自动切换模型/读快照） |
| `hardening_protocol` | 完整协议说明 |

## 状态文件

`{workspace}/.iterative-hardening/session.json`

## Cursor 配置

```json
"iterative-hardening": {
  "command": "node",
  "args": ["D:\\0HAN\\HANDAILY\\mcp\\iterative-hardening\\dist\\index.js"],
  "env": {
    "HANDAILY_ROOT": "D:\\0HAN\\HANDAILY"
  }
}
```

## 开发

```bash
cd mcp/iterative-hardening
npm install
npm run build
```

## 循环形状

```
hardening_init → LOOP:
  hardening_next_pass
  → REPRODUCE → AUDIT → FIX → VERIFY → RETEST (全矩阵)
  → hardening_submit_pass
  → hardening_can_finish
```
