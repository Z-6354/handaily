# 舰娘 Cubism 播放器页 — 设计

> **废止（UI）**：独立「舰娘」侧栏页已移除（2026-07-14）。入口改为人物 → 皮肤；桌宠路径见 [kanmusu-desktop-pet](./2026-07-14-kanmusu-desktop-pet-design.md)。

**日期**: 2026-07-14  
**状态**: 舰娘页目标废止；Cubism 能力已合入统一皮肤 / 桌宠

## 目标

在保留现有人物/桌宠逻辑不变的前提下，新增「舰娘」页：管理角色与 Cubism Live2D 皮肤，并用**独立带边框窗口**播放，默认关闭。同时提供「删除全部非内置桌宠模型」能力。

## 非目标（v1）

> **2026-07-14 翻转**：日常路径已改为与 Spine 共用 `pet` 桌宠窗，见 [kanmusu-desktop-pet-design](./2026-07-14-kanmusu-desktop-pet-design.md)。下文「独立预览窗」仅作历史记录。

- ~~不做舰娘桌宠 / 系统托盘跟随~~（已做桌宠路径）
- ~~不与桌宠 Spine 共用展示面板或运行时~~（互斥共用 `pet` 窗）
- 不完整移植 Cubism 全量 motion3 流水线（可用 idle/手动切 AnimationClip 名 + 静态姿）
- 不替换现有人物页

## 架构

```
主窗口 舰娘页（列表/简介/皮肤/台词）
    │ IPC: open / close / load_skin
    ▼
独立窗口 kanmusu-player（带边框，默认不创建）
    └── Cubism Web 运行时加载 .moc3 + model3.json + textures
```

数据与桌宠分离：

| | 桌宠 / 人物页 | 舰娘页 |
|--|--|--|
| 模型 | Spine `pet-models/` | Cubism `kanmusu-models/` ← 同步自 `data/model/unpacked` |
| Manifest | `characters/manifest.json` | `kanmusu/manifest.json` |
| 台词 | 多在 persona / 模型 meta | **挂在每个皮肤** |
| 预览 | pet 窗口 | **独立 kanmusu-player 窗口** |

## 数据模型

```json
{
  "version": 1,
  "characters": [
    {
      "id": "aidang",
      "name": "爱宕",
      "description": "简介…",
      "skins": [
        {
          "id": "aidang_2",
          "name": "皮肤2",
          "model_dir": "aidang_2",
          "lines": [{ "text": "……", "animation": "touch_body" }]
        }
      ]
    }
  ]
}
```

首包种子：扫描 `data/model/unpacked/*`（及 AppData 同步目录）中含 `.moc3` 的 slug，按拼音 base 聚合成角色。

## 播放器窗口

- Tauri 独立 label：`kanmusu-player`
- 有边框、可调整大小；默认更大（如 900×1200）
- 平时不创建；舰娘页点「预览」才 `show`
- 关闭仅隐藏，不毁主进程
- 加载路径：convertFileSrc → Cubism runtime

## 「删除全部」

- 放在人物页或桌宠设置相关区域：一键删除**全部非内置 Spine 模型**（确认对话框）
- 不删除内置 chaijun/edu/…；不影响舰娘 Cubism 库存（舰娘另有自己的删除入口，v1 可后做）

## 验收

1. 侧栏出现「舰娘」，原人物/设置/帮助仍在  
2. 能看到 hanimport 解包的若干 Cubism 皮肤并选中  
3. 「预览」打开独立边框窗口播放；默认不打开  
4. 角色有简介；台词编辑保存在皮肤下  
5. 「删除全部非内置模型」清空用户 Spine 包，内置保留  
