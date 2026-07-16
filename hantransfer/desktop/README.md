# hantransfer-desktop

Windows 端：mDNS 广播 + axum HTTP 服务 + 系统托盘。

## 模块

| 文件 | 职责 |
|------|------|
| `main.rs` | 入口、CLI |
| `config.rs` | 设备名、端口、数据路径 |
| `discovery.rs` | mDNS 注册与浏览 |
| `server.rs` | HTTP API 路由 |
| `trust.rs` | 设备信任表 |
| `transfer.rs` | multipart 接收与 SHA256 校验 |
| `importer.rs` | 按 type 路由收件子目录 |
| `tray.rs` | 系统托盘菜单 |

## 开发

```bash
cargo run -p hantransfer-desktop
cargo test -p hantransfer-desktop
```

## API

见 [../proto/api.yaml](../proto/api.yaml)。
