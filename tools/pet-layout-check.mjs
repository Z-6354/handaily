#!/usr/bin/env node
/** @typedef {"bottom-center"|"top-left"} StageOrigin */

/**
 * @param {number} tx
 * @param {number} ty
 * @param {number} scale
 * @param {number} stageW
 * @param {number} stageH
 * @param {StageOrigin} from
 * @param {StageOrigin} to
 */
function convertStageOffsetForOrigin(tx, ty, scale, stageW, stageH, from, to) {
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

function isCursorOutsideWindow(cursorX, cursorY, bounds, handleOutset = 12) {
  return (
    cursorX < bounds.x - handleOutset ||
    cursorX >= bounds.x + bounds.width + handleOutset ||
    cursorY < bounds.y - handleOutset ||
    cursorY >= bounds.y + bounds.height + handleOutset
  );
}

let failed = 0;

function assert(name, cond) {
  if (!cond) {
    console.error("FAIL:", name);
    failed += 1;
  } else {
    console.log("ok:", name);
  }
}

const W = 240;
const H = 320;
const bc = { x: 10, y: 20 };
const tl = convertStageOffsetForOrigin(bc.x, bc.y, 0.8, W, H, "bottom-center", "top-left");
const back = convertStageOffsetForOrigin(tl.x, tl.y, 0.8, W, H, "top-left", "bottom-center");
assert("offset round-trip BC→TL→BC", back.x === bc.x && back.y === bc.y);

const bounds = { x: 100, y: 200, width: 240, height: 320 };
assert("N handle inside (top edge -4)", !isCursorOutsideWindow(200, 196, bounds));
assert("far outside desktop", isCursorOutsideWindow(50, 50, bounds));

/** 与 spinePet.resizeCanvasForEditResize 一致：canvas 内位移 = delta / stageScale */
function spineEditResizeShift(dw, dh, stageScale) {
  const s = Math.max(0.01, stageScale);
  return { x: dw / s, y: dh / s };
}
const west = spineEditResizeShift(-40, 0, 1.2);
assert("west resize shift at scale 1.2", west.x === -40 / 1.2 && west.y === 0);
const north = spineEditResizeShift(0, 30, 0.8);
assert("north resize shift at scale 0.8", north.x === 0 && north.y === 30 / 0.8);

process.exit(failed > 0 ? 1 : 0);
