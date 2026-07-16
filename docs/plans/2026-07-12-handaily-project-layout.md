# HANDAILY 项目布局实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** HANDAILY 项目级 monorepo — `hanpet`（桌宠应用）、`hanimport`（开发导入器）、`data/`（工作数据），其余为项目级共享内容。

**Architecture:** 两应用解耦；`data/` 集中开发期 Spine/BWIKI/计划文件；hanpet 源码与构建产物均在 `hanpet/`（`hanpet/dist/` 为唯一前端输出）。

**Tech Stack:** Rust (hanpet Tauri + hanimport CLI)、TypeScript (MCP)、Unity bundle 解析（hanimport Phase 1 选型）。

## Global Constraints

- Phase 0 仅脚手架 + 文档，不破坏根目录 `npm run tauri:dev`
- hanimport 不进 hanpet 发行包
- 默认工作路径：`data/live2d/`、`data/wiki/`、`data/import/`
- 兼容旧 `仓库根/live2d/`（本仓库已迁移完成）
- Windows 10/11 主平台

---

## 目标结构

```
HANDAILY/
├── hanpet/                 # 小寒桌宠
├── hanimport/              # 小寒导入器
├── data/
│   ├── live2d/
│   ├── wiki/
│   ├── import/
│   └── game/
├── mcp/
├── packages/
├── docs/
└── scripts/
```

---

## Phase 0 — 架构脚手架 ✅

- [x] 创建 `hanpet/`、`hanimport/`、`data/` 及子目录
- [x] `docs/ARCHITECTURE.md`、各 README、hanimport DESIGN
- [x] 更新根 README
- [x] 路径解析支持 `data/live2d`（兼容 `live2d/`；仓库内已迁移 1557 slug）
- [x] 移除旧 `apps/` 布局

---

## Phase 1 — hanimport unpack

**Files:** `Cargo.toml`, `hanimport/`, `hanimport/src/unpack/`

- [x] Rust workspace 根配置（`Cargo.toml` + `hanimport` + `hanpet/src-tauri` members）
- [x] `hanimport unpack --input --output --dry-run` CLI 骨架
- [ ] AssetBundle 解析 POC → `data/live2d/<slug>/`
- [ ] 验收：`blhx_scan_live2d` 可扫描新解包目录

---

## Phase 2 — 导入 CLI 归入 hanimport

- [x] `hanimport plan` ← `mcp/blhx-wiki` live2d-plan（薄包装）
- [x] `hanimport models` ← `live2d_import`（薄包装）
- [x] `hanimport personas` ← `blhx_import`（薄包装）
- [x] `hanimport roster export` ← `roster_pack`（薄包装）
- [ ] 将 bin 逻辑迁入 hanimport crate（去 cargo 子进程）
- [ ] 默认计划路径 `data/import/live2d-plan.json` 文档化完成

---

## Phase 3 — hanpet 迁入 ✅

- [x] `src/`、`src-tauri/`、`bundled/`、`public/`、`package.json` → `hanpet/`
- [x] 根 `package.json` workspaces
- [x] 更新 CI / 脚本路径
- [x] 前端构建产物仅输出 `hanpet/dist/`（删除根目录残留 `dist/`）

---

## Phase 4 — 共享库

- [ ] `packages/blhx-slug-match` ← `mcp/blhx-wiki/src/live2d.ts`
- [ ] `crates/handaily-spine-pack` ← `pet/models.rs`

---

## 任务文件

- `.specs/tasks/draft/restructure-handaily-monorepo.chore.md`
- `.specs/tasks/draft/implement-hanimport-app.feature.md`

## 已废弃

旧 `apps/xiaohan-daily`、`apps/blhx-unpack` 布局 — 见 git 历史及 `docs/plans/2026-07-12-handaily-monorepo-blhx-unpack.md`。
