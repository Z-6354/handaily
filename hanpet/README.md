# hanpet — 小寒桌宠

Tauri 2 + Rust + React 桌面桌宠应用。

## 目录

```
hanpet/
├── index.html / pet.html / pet-menu.html   # Vite 多页入口
├── src/                                    # React 前端
├── src-tauri/                              # Rust 后端
├── bundled/                                # 内置 roster / prompts
├── public/
├── scripts/                                # 本应用构建脚本
├── dist/                                   # Vite 构建产物（gitignore）
└── package.json
```

## 开发

在**仓库根目录**执行（推荐）：

```bash
npm install
npm run tauri:dev
```

或进入本目录：

```bash
cd hanpet
npm run tauri:dev
```

## 打包

```bash
npm run build:release    # 仓库根
```

产物：`hanpet/src-tauri/target/release/xiaohan-daily.exe`

## 职责

| 属于 hanpet | 项目级（仓库根） |
|-------------|------------------|
| 人物页（皮肤 = 桌宠 Spine + 舰娘 Cubism + 台词） | `mcp/`、`data/`、`hanimport/` |
| 桌宠 pet 窗 / 菜单、托盘、设置 | `scripts/build.ps1`、`.cargo/` |
| `%AppData%/xiaohan-daily/data/` | 本地个人库 `data/roster/`（不入包） |
| 自带预览 `bundled/roster/` | `npm run roster:publish` 白名单导出 |
