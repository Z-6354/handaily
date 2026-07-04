# 45 · OpenCode GO 供应商 Base URL 修复

**日期**：2026-07-02  
**类型**：Bug 修复 / AI 供应商

## 问题

OpenCode GO 套餐连接测试失败：

```
解析模型列表失败: 无效 JSON: expected value at line 1 column 1
```

## 根因

供应商配置使用了错误的 Base URL `https://api.opencode.ai/v1`。该地址 `/v1/models` 返回纯文本 `Not Found`（HTTP 200），不是 JSON。

OpenCode GO 正确网关为 **`https://opencode.ai/zen/go/v1`**（官方文档 [Go | OpenCode](https://opencode.ai/docs/go/)）。

## 修复

- `config/vendors.json`：`opencode.base_url` 改为 `https://opencode.ai/zen/go/v1`
- 增加默认模型 `deepseek-v4-pro` 与 auth 提示
- `catalog.load` 自动迁移旧 URL `api.opencode.ai/v1` → 新 URL
- 模型列表非 JSON 响应时给出明确错误（含 Base URL 提示）

## 用户操作

1. **重启应用**（加载新 vendors 配置与迁移逻辑）
2. 设置 → AI 配置 → OpenCode GO → 确认密钥来自 opencode.ai Go 套餐
3. 点击「测试」应能拉取约 20 个模型；或手动添加如 `deepseek-v4-pro`、`kimi-k2.7-code`
