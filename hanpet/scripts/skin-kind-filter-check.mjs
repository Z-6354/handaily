#!/usr/bin/env node
/** Mirrors hanpet/src/lib/skinKindFilter.ts for CI-less smoke checks. */

function skinMatchesKind(skin, kind) {
  if (kind === "spine") {
    return Boolean(skin.model_id?.trim()) || Boolean(skin.model_ready);
  }
  const dir = skin.kanmusu_dir?.trim();
  return Boolean(dir) || Boolean(skin.kanmusu_ready);
}

function filterSkinsByKind(skins, kind) {
  return skins.filter((s) => skinMatchesKind(s, kind));
}

let failed = 0;
function assert(name, cond) {
  if (!cond) {
    console.error(`FAIL ${name}`);
    failed += 1;
  } else {
    console.log(`ok   ${name}`);
  }
}

const skins = [
  { id: "a", model_id: "lafei", model_ready: true, kanmusu_dir: "lafei", kanmusu_ready: true },
  { id: "b", model_id: "only_pet", model_ready: true, kanmusu_dir: "", kanmusu_ready: false },
  { id: "c", model_id: "", model_ready: false, kanmusu_dir: "only_km", kanmusu_ready: true },
  { id: "d", model_id: "pending", model_ready: false, kanmusu_dir: "", kanmusu_ready: false },
];

const spine = filterSkinsByKind(skins, "spine").map((s) => s.id);
const kanmusu = filterSkinsByKind(skins, "kanmusu").map((s) => s.id);

assert("spine includes dual + pet-only + pending model_id", spine.join(",") === "a,b,d");
assert("spine excludes kanmusu-only", !spine.includes("c"));
assert("kanmusu includes dual + kanmusu-only", kanmusu.join(",") === "a,c");
assert("kanmusu excludes pet-only", !kanmusu.includes("b"));

if (failed) {
  console.error(`\n${failed} failed`);
  process.exit(1);
}
console.log("\nskin-kind-filter-check: all passed");
