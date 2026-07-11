# handaily-pet MCP

通过 MCP 控制小寒桌宠（台词、换模、快照等），底层调用应用内 HTTP 控制面。

## 前置条件

1. 在应用 **设置 → 启动 → Agent 控制接口 (MCP)** 中开启（会重启应用）
2. 应用运行中，本机 `http://127.0.0.1:19420` 可访问

## 安装

```bash
cd mcp/handaily-pet
npm install
npm run build
```

## Cursor MCP 配置示例

```json
{
  "mcpServers": {
    "handaily-pet": {
      "command": "node",
      "args": ["D:/0HAN/HANDAILY/mcp/handaily-pet/dist/index.js"],
      "env": {
        "HANDAILY_TEST_API_URL": "http://127.0.0.1:19420"
      }
    }
  }
}
```

## 工具

| 工具 | 说明 |
|------|------|
| `pet_control` | 统一入口，`action` 见下表 |

### `pet_control` actions

- `health`, `index`, `snapshot`, `status`, `skins`, `characters`, `favorites`, `logs`
- `speak` — 需要 `text`，可选 `animation`
- `speak_random` — 随机台词
- `preview_animation` — 需要 `animation`，可选 `loop`
- `switch_next_skin`, `switch_next_character`, `switch_skin`, `switch_character`
- `menu_open`, `menu_hide`, `edit_enter`
- `main_open`, `main_close`, `bubble_set`, `interaction`
- `cursor_get`, `cursor_set`, `mouse_click`, `screenshot`, `screenshot_pet` — Windows 系统级 UI 自动化（需 debug + test-api）
