export type StageOrigin = "bottom-center" | "top-left";

/** DB 与正常模式使用 bottom-center；编辑模式使用 top-left */
export function convertStageOffsetForOrigin(
  tx: number,
  ty: number,
  scale: number,
  stageW: number,
  stageH: number,
  from: StageOrigin,
  to: StageOrigin,
): { x: number; y: number } {
  if (from === to) return { x: tx, y: ty };
  const ox = stageW / 2;
  const oy = stageH;
  if (from === "bottom-center" && to === "top-left") {
    return {
      x: Math.round(tx + ox * (1 - scale)),
      y: Math.round(ty + oy * (1 - scale)),
    };
  }
  return {
    x: Math.round(tx - ox * (1 - scale)),
    y: Math.round(ty - oy * (1 - scale)),
  };
}

/** 光标是否在窗口外（含手柄延伸容差） */
export function isCursorOutsideWindow(
  cursorX: number,
  cursorY: number,
  bounds: { x: number; y: number; width: number; height: number },
  handleOutset = 12,
): boolean {
  return (
    cursorX < bounds.x - handleOutset ||
    cursorX >= bounds.x + bounds.width + handleOutset ||
    cursorY < bounds.y - handleOutset ||
    cursorY >= bounds.y + bounds.height + handleOutset
  );
}
