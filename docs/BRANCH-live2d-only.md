# 纯 Live2D 分支说明 (feat/live2d-only)

> 三分支总览见 [BRANCHES.md](BRANCHES.md)。

## 定位

纯桌宠：**模型 + Live2D + wiki 台词**。无日报、无 AI 汇报、无微信。

## 已落地

### 模块物理删除

已移除：`analysis/`、`ai/`、`report/`、`wechat/`、`agent_http/`、`timeline/`、`screenshot/`、`work_type/`、`vault/`、`ipc/commands.rs`

保留 stub：`live2d/analysis_stub.rs`、`live2d/vault_stub.rs`

IPC 入口：`ipc/live2d_commands.rs`（76 个命令）

### Cargo 瘦身

已移除：`aes-gcm`、`pbkdf2`、`sha2`、`rand`、`qrcode`、`notify`

保留：`reqwest`（wiki 爬取）、`image`（应用图标）、`base64`

### 功能

- 人设/wiki 导入：**本地解析**，不调用 AI（`persona/import_reference.rs` 重写）
- 台词：wiki 结构化解析 + `local_extract_lines`，无 AI 清洗
- 后端/前端/IPC/托盘：见上一版说明

## 验证

```bash
git checkout feat/live2d-only
npm run check:rust
npm run tauri:dev
```

## 可选后续

- 二进制改名 `xiaohan-pet.exe`
- 精简 DB migrate（跳过 insights/periods/reports 表）
