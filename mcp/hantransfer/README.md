# hantransfer MCP

Cursor Agent 通过本机 `hantransfer-desktop`（默认 `http://127.0.0.1:7822`）查看设备并推送/接受文件。

## 前置

```powershell
npm run hantransfer
# 或
.\scripts\start-hantransfer.ps1
```

## 构建

```powershell
npm install
npm run build -w @handaily/hantransfer-mcp
```

## Cursor MCP 配置示例

```json
{
  "mcpServers": {
    "hantransfer": {
      "command": "node",
      "args": ["mcp/hantransfer/dist/index.js"],
      "env": {
        "HANTRANSFER_URL": "http://127.0.0.1:7822"
      }
    }
  }
}
```

## 工具

| 工具 | 说明 |
|------|------|
| `snapshot` | 完整状态 |
| `list_devices` | 已信任设备 |
| `push_files` | `{ device_id, paths[] }` 推送到手机 |
| `accept_receive` | 接受待收（可选 `id`） |

管理页「AI 状态」面板同源：`GET /api/v1/agent/snapshot`。
