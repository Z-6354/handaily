# hantransfer — 小寒传文件

HANDAILY monorepo 下的**局域网文件桥接工具**（Windows + Android），不进 hanpet 发行包。

## 架构

```
手机（浏览器 / APK）
    │ mDNS 发现 + HTTP multipart
    ▼
hantransfer-desktop（Windows）
    │ 信任 / SHA256 / 流式收件
    ▼
data/transfer/inbox/          ← 通用文件
data/transfer/inbox/azurlane/ ← 碧蓝 AssetBundles
    │ 可选 hanimport unpack
    ▼
data/live2d/ → hanpet
```

| 目录 | 职责 |
|------|------|
| `desktop/` | Rust 托盘 + axum HTTP 服务 |
| `mobile-web/` | 手机 UI（浏览器与 APK WebView 共用） |
| `android-maven/` | Maven 构建 WebView APK（无需 Android Studio） |
| `proto/` | OpenAPI + JSON Schema + Bridge 协议 |

## 开发环境

| 组件 | 要求 |
|------|------|
| PC | Rust 1.82+、Windows 10/11 |
| 浏览器模式 | 无额外依赖 |
| APK 构建 | Maven 3.9+、`ANDROID_HOME`、SDK platform 34 |
| 可选联动 | `hanimport` crate（同 monorepo） |

## Agent / Cursor MCP

管理页顶部有 **AI 状态** 面板。本机 API（仅 localhost）：

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/agent/snapshot` | 设备 + 收件 + 推送队列 |
| POST | `/api/v1/agent/push` | `{ "device_id", "paths": ["绝对路径"] }` |
| POST | `/api/v1/agent/receive/accept` | `{ "id"? }` 省略则全部接受 |

MCP：`mcp/hantransfer/`（见该目录 README）。

## 启动

```bash
# PC 控制台
npm run hantransfer

# PC 托盘（Release 无控制台窗口）
npm run hantransfer -- --tray

# 仅开发：自动信任新设备
npm run hantransfer -- --auto-trust
```

手机浏览器打开 PC 日志中的 **`http://<LAN_IP>:7822/m/`**（同 WiFi 可自动连接）。

APK（构建后**自动发布**到 `release/`，供手机 LAN 自动更新）：

```powershell
npm run hantransfer:apk
# 输出 hantransfer/release/hantransfer-0.1.0-buildN-debug.apk + latest.json

adb install -r hantransfer/release/hantransfer-latest-debug.apk
```

每次改完 App **只需再跑一次** `npm run hantransfer:apk`，build 号自动 +1 并写入 `latest.json`。PC 端 `npm run hantransfer` 运行中即可被手机检测到更新。

管理页 `http://127.0.0.1:7822/` → **手机 App 更新** 也可手动选择/上传 APK。

## 协议

完整定义见 [`proto/api.yaml`](proto/api.yaml)。

### 统一响应格式

成功：

```json
{ "ok": true, "data": { ... } }
```

失败：

```json
{ "ok": false, "error": { "code": "UPLOAD_FAILED", "message": "..." } }
```

### 核心端点

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/status` | 设备状态（`/api/v1/info` 别名） |
| POST | `/api/v1/handshake` | 首次信任握手 |
| POST | `/api/v1/files` | multipart 上传（metadata + file） |
| POST | `/api/v1/files/check` | 同名同大小预检（存在则跳过上传） |
| GET | `/api/v1/transfers/:id` | 传输进度 |
| GET | `/api/v1/trust` | 已信任设备列表 |
| DELETE | `/api/v1/trust/:id` | 撤销信任（仅 localhost） |
| POST | `/api/v1/push` | 电脑推送到手机（localhost，multipart） |
| GET | `/api/v1/push/outbox` | 电脑端查看待接收队列（localhost） |
| DELETE | `/api/v1/push/{id}` | 取消未接收的推送（localhost） |
| GET | `/api/v1/push/pending` | 手机拉取待接收列表 |
| GET | `/api/v1/push/{id}/file` | 手机下载推送文件 |
| POST | `/api/v1/push/{id}/ack` | 手机确认已接收 |
| GET | `/api/v1/app/release` | 手机查询最新 APK 版本 |
| GET | `/api/v1/app/release/download` | 手机下载最新 APK |
| GET | `/api/v1/app/release/list` | 列出 release 目录 APK（localhost） |
| POST | `/api/v1/app/release/latest` | 指定 APK 为最新版（localhost） |
| POST | `/api/v1/app/release/upload` | 上传 APK 并设为最新（localhost） |
| GET | `/m/` | 手机 Web UI |

Bridge 协议：[`proto/mobile-bridge.json`](proto/mobile-bridge.json)

## 环境变量

| 变量 | 说明 |
|------|------|
| `HANDAILY_ROOT` | monorepo 根目录 |
| `HANTRANSFER_PORT` | 监听端口（默认 7822） |
| `HANTRANSFER_DEVICE_NAME` | mDNS 显示名 |
| `HANTRANSFER_INBOX_DIR` | 收件目录 |
| `HANTRANSFER_HISTORY_DIR` | 传输记录目录 |
| `HANTRANSFER_TEMP_DIR` | 临时文件目录 |
| `HANTRANSFER_OUTBOX_DIR` | 电脑→手机推送队列目录 |
| `HANTRANSFER_AUTO_TRUST` | `1` 时自动信任 |
| `HANTRANSFER_AUTO_HANIMPORT` | `1` 时收到 `azurlane_asset` 后后台运行 `hanimport unpack` |

## 工作流

1. PC 启动 `hantransfer`，手机打开 `/m/` 或安装 APK
2. 首次连接在 PC 浏览器管理页（`http://127.0.0.1:7822/`）确认信任
3. **浏览器**可发送相册/下载等普通文件；**碧蓝航线 AssetBundles** 等 App 私有目录需安装 APK，在「碧蓝」页授权后批量发送
4. **电脑 → 手机**：在 PC 管理页选择已信任设备，可**多选文件**批量推送；手机在「接收」页下载（APK 自动接收，支持全部下载）
5. 发送文件 → PC 流式写入 `temp/` → SHA256 校验 → 移入 `inbox/`
6. PC 弹出系统通知；碧蓝资源可运行 `hanimport unpack --input data/transfer/inbox/azurlane/<category>`

## 设计文档

[docs/plans/hantransfer-design.md](../docs/plans/hantransfer-design.md)

## 测试

```bash
npm run test:hantransfer
cargo test -p hantransfer-desktop
```

手动验收（需真机同 WiFi）：发现 PC → 握手 → 上传 1MB/100MB → 收到通知 → AssetBundles 批量发送。
