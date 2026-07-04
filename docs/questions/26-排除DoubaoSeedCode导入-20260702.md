# 26 · 排除火山 Doubao Seed Code 导入项

**日期**：2026-07-02  
**类型**：配置调整

## 需求

移除下拉中的「Doubao Seed Code (已导入)」。

## 实现

- `vendors.json` → `volcano.excluded_models`: `["doubao-seed-code"]`
- 导入时过滤排除列表
- 启动加载时从 `imported_models` 清理；若当前选中该模型则清空并回退默认

📁 已归档：`docs/questions/26-排除DoubaoSeedCode导入-20260702.md`
